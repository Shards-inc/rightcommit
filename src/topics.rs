//! Personal workflow organization, kept separate from graphs and run snapshots.
use crate::{
    App,
    auth::{ApiResult, bad},
    model::User,
    store::Store,
};
use axum::{Json, extract::State};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

const UNASSIGNED: &str = "unassigned";
#[derive(Clone, Serialize, Deserialize)]
pub struct Topic {
    pub id: String,
    pub name: String,
}
#[derive(Serialize, Deserialize)]
pub struct Organization {
    pub topics: Vec<Topic>,
    pub assignments: BTreeMap<String, String>,
    pub recent: Vec<String>,
}
impl Default for Organization {
    fn default() -> Self {
        Self {
            topics: vec![Topic {
                id: UNASSIGNED.into(),
                name: "Unassigned".into(),
            }],
            assignments: BTreeMap::new(),
            recent: vec![],
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum Action {
    Create {
        name: String,
    },
    Rename {
        topic: String,
        name: String,
    },
    Remove {
        topic: String,
    },
    Reorder {
        topics: Vec<String>,
    },
    Move {
        workflows: Vec<String>,
        topic: String,
    },
    Open {
        workflow: String,
    },
}
fn topic_name(name: &str, state: &Organization, except: &str) -> anyhow::Result<String> {
    let name = name.trim();
    anyhow::ensure!(
        !name.is_empty() && name.chars().count() <= 80 && !name.chars().any(char::is_control),
        "Nome topic non valido (1–80 caratteri)"
    );
    anyhow::ensure!(
        !state
            .topics
            .iter()
            .any(|t| t.id != except && t.name.to_lowercase() == name.to_lowercase()),
        "Topic già esistente"
    );
    Ok(name.into())
}
impl Store {
    pub fn organization(&self, uid: i64, action: Option<Action>) -> anyhow::Result<Organization> {
        let mut db = self.db.lock().unwrap();
        let tx = db.transaction()?;
        let raw: Option<String> = tx
            .query_row(
                "SELECT json FROM workflow_organization WHERE user_id=?",
                [uid],
                |r| r.get(0),
            )
            .optional()?;
        let mut state: Organization = raw
            .map(|s| serde_json::from_str(&s))
            .transpose()?
            .unwrap_or_default();
        let workflows: HashSet<String> = {
            let mut stmt = tx.prepare("SELECT id FROM workflows WHERE user_id=?")?;
            stmt.query_map([uid], |r| r.get(0))?
                .collect::<Result<_, _>>()?
        };
        state.assignments.retain(|id, _| workflows.contains(id));
        state.recent.retain(|id| workflows.contains(id));
        let exists = |id: &str| state.topics.iter().any(|t| t.id == id);
        match action {
            Some(Action::Create { name }) => {
                anyhow::ensure!(state.topics.len() < 200, "Limite topic raggiunto");
                let name = topic_name(&name, &state, "")?;
                state.topics.push(Topic {
                    id: crate::model::id(),
                    name,
                });
            }
            Some(Action::Rename { topic, name }) => {
                anyhow::ensure!(
                    topic != UNASSIGNED && exists(&topic),
                    "Topic non modificabile"
                );
                let name = topic_name(&name, &state, &topic)?;
                state
                    .topics
                    .iter_mut()
                    .find(|t| t.id == topic)
                    .unwrap()
                    .name = name;
            }
            Some(Action::Remove { topic }) => {
                anyhow::ensure!(
                    topic != UNASSIGNED && exists(&topic),
                    "Topic non rimovibile"
                );
                state.topics.retain(|t| t.id != topic);
                state.assignments.retain(|_, t| t != &topic);
            }
            Some(Action::Reorder { topics }) => {
                anyhow::ensure!(
                    topics.len() == state.topics.len()
                        && topics.iter().collect::<HashSet<_>>().len() == topics.len()
                        && topics.iter().all(|t| exists(t)),
                    "Ordine topic non valido; aggiorna la lista"
                );
                state
                    .topics
                    .sort_by_key(|t| topics.iter().position(|id| id == &t.id).unwrap());
            }
            Some(Action::Move {
                workflows: ids,
                topic,
            }) => {
                anyhow::ensure!(
                    exists(&topic)
                        && ids.len() <= 2000
                        && ids.iter().all(|id| workflows.contains(id)),
                    "Topic o workflow non trovato"
                );
                for id in ids {
                    if topic == UNASSIGNED {
                        state.assignments.remove(&id);
                    } else {
                        state.assignments.insert(id, topic.clone());
                    }
                }
            }
            Some(Action::Open { workflow }) => {
                anyhow::ensure!(workflows.contains(&workflow), "Workflow non trovato");
                state.recent.retain(|id| id != &workflow);
                state.recent.insert(0, workflow);
            }
            None => return Ok(state),
        }
        tx.execute("INSERT INTO workflow_organization(user_id,json) VALUES(?,?) ON CONFLICT(user_id) DO UPDATE SET json=excluded.json", params![uid, serde_json::to_string(&state)?])?;
        tx.commit()?;
        Ok(state)
    }
}
pub async fn read(State(a): State<App>, u: User) -> ApiResult<Json<Organization>> {
    Ok(Json(a.store.organization(u.id, None)?))
}
pub async fn edit(
    State(a): State<App>,
    u: User,
    Json(action): Json<Action>,
) -> ApiResult<Json<Organization>> {
    Ok(Json(
        a.store
            .organization(u.id, Some(action))
            .map_err(|e| bad(e.to_string()))?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn setup() -> (tempfile::TempDir, Store) {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        for uid in [1, 2] {
            let w = serde_json::from_value(serde_json::json!({"id":"w", "name":"Workflow", "description":"", "entry":"", "nodes":[], "edges":[]})).unwrap();
            store.save_workflow(uid, &w).unwrap();
        }
        (dir, store)
    }
    #[test]
    fn topics_migrate_isolate_and_preserve_graphs() {
        let (_dir, store) = setup();
        let before = serde_json::to_value(store.workflow(1, "w").unwrap()).unwrap();
        assert_eq!(
            store.organization(1, None).unwrap().topics[0].name,
            "Unassigned"
        );
        let state = store
            .organization(
                1,
                Some(Action::Create {
                    name: "Project".into(),
                }),
            )
            .unwrap();
        let topic = state.topics[1].id.clone();
        store
            .organization(
                1,
                Some(Action::Move {
                    workflows: vec!["w".into()],
                    topic: topic.clone(),
                }),
            )
            .unwrap();
        store
            .organization(
                1,
                Some(Action::Open {
                    workflow: "w".into(),
                }),
            )
            .unwrap();
        assert_eq!(store.organization(1, None).unwrap().assignments["w"], topic);
        assert_eq!(store.organization(2, None).unwrap().topics.len(), 1);
        assert!(
            store
                .organization(
                    2,
                    Some(Action::Move {
                        workflows: vec!["w".into()],
                        topic: topic.clone()
                    })
                )
                .is_err()
        );
        assert!(
            store
                .organization(
                    1,
                    Some(Action::Move {
                        workflows: vec!["w".into(), "missing".into()],
                        topic: topic.clone()
                    })
                )
                .is_err()
        );
        store
            .organization(1, Some(Action::Remove { topic }))
            .unwrap();
        let state = store.organization(1, None).unwrap();
        assert!(state.assignments.is_empty());
        assert_eq!(state.recent, vec!["w"]);
        assert_eq!(
            serde_json::to_value(store.workflow(1, "w").unwrap()).unwrap(),
            before
        );
    }
    #[test]
    fn topics_order_validation_and_reopening() {
        let (dir, store) = setup();
        let state = store
            .organization(
                1,
                Some(Action::Create {
                    name: "Research".into(),
                }),
            )
            .unwrap();
        let topic = state.topics[1].id.clone();
        assert!(
            store
                .organization(
                    1,
                    Some(Action::Create {
                        name: " research ".into()
                    })
                )
                .is_err()
        );
        assert!(
            store
                .organization(
                    1,
                    Some(Action::Remove {
                        topic: UNASSIGNED.into()
                    })
                )
                .is_err()
        );
        assert!(
            store
                .organization(
                    1,
                    Some(Action::Rename {
                        topic: UNASSIGNED.into(),
                        name: "Other".into()
                    })
                )
                .is_err()
        );
        assert!(
            store
                .organization(
                    1,
                    Some(Action::Reorder {
                        topics: vec![topic.clone(), topic.clone()]
                    })
                )
                .is_err()
        );
        store
            .organization(
                1,
                Some(Action::Reorder {
                    topics: vec![topic.clone(), UNASSIGNED.into()],
                }),
            )
            .unwrap();
        for _ in 0..2 {
            store
                .organization(
                    1,
                    Some(Action::Open {
                        workflow: "w".into(),
                    }),
                )
                .unwrap();
        }
        drop(store);
        let store = Store::open(dir.path()).unwrap();
        let state = store.organization(1, None).unwrap();
        assert_eq!(state.topics[0].id, topic);
        assert_eq!(state.recent.len(), 1);
        store
            .db
            .lock()
            .unwrap()
            .execute("DELETE FROM workflows WHERE user_id=1", [])
            .unwrap();
        assert!(store.organization(1, None).unwrap().recent.is_empty());
    }
}
