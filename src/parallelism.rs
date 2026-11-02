use crate::store::Store;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;

/// Admission is per user. Reducing the limit never cancels an active workflow.
#[derive(Default)]
pub struct Scheduler {
    active: Mutex<HashMap<i64, usize>>,
    pub changed: Notify,
    #[cfg(test)]
    limit: Option<usize>,
}
pub struct Permit {
    scheduler: Arc<Scheduler>,
    uid: i64,
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut active = self.scheduler.active.lock().unwrap();
        if let Some(count) = active.get_mut(&self.uid) {
            *count = count.saturating_sub(1);
        }
        drop(active);
        self.scheduler.changed.notify_waiters();
    }
}
impl Scheduler {
    #[cfg(test)]
    pub fn limited(limit: usize) -> Self {
        Self {
            limit: Some(limit),
            ..Default::default()
        }
    }
    pub async fn acquire(self: &Arc<Self>, store: &Store, uid: i64) -> Permit {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let limit = store
                .settings(uid)
                .map(|s| s.workflow_parallelism.max(1))
                .unwrap_or(5);
            #[cfg(test)]
            let limit = self.limit.unwrap_or(limit);
            {
                let mut active = self.active.lock().unwrap();
                let count = active.entry(uid).or_default();
                if *count < limit {
                    *count += 1;
                    return Permit {
                        scheduler: self.clone(),
                        uid,
                    };
                }
            }
            changed.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;
    #[tokio::test]
    async fn personal_limit_defaults_to_five_and_changes_without_cancelling_active_runs() {
        let dir = tempfile::tempdir().unwrap();
        let app = crate::tests::app(dir.path());
        let mut held = Vec::new();
        for _ in 0..5 {
            held.push(app.slots.acquire(&app.store, 1).await);
        }
        // Another user's workflows have independent admission.
        let other =
            tokio::time::timeout(Duration::from_millis(100), app.slots.acquire(&app.store, 2))
                .await
                .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), app.slots.acquire(&app.store, 1))
                .await
                .is_err()
        );
        let mut settings = app.store.settings(1).unwrap();
        settings.workflow_parallelism = 6;
        app.store.save_settings(1, &settings).unwrap();
        app.slots.changed.notify_waiters();
        held.push(
            tokio::time::timeout(Duration::from_millis(100), app.slots.acquire(&app.store, 1))
                .await
                .unwrap(),
        );
        settings.workflow_parallelism = 4;
        app.store.save_settings(1, &settings).unwrap();
        assert_eq!(app.slots.active.lock().unwrap()[&1], 6);
        held.pop();
        held.pop();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), app.slots.acquire(&app.store, 1))
                .await
                .is_err()
        );
        held.pop();
        let resumed =
            tokio::time::timeout(Duration::from_millis(100), app.slots.acquire(&app.store, 1))
                .await
                .unwrap();
        drop((held, resumed, other));
        assert_eq!(app.slots.active.lock().unwrap()[&1], 0);
    }
}
