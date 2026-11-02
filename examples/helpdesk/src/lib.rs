//! Minimal existing helpdesk domain. Lower priority numbers are more urgent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub id: u64,
    pub fingerprint: String,
    pub priority: u8,
    pub occurrences: u32,
}

/// Existing V1 behavior preserves the older ticket's priority.
pub fn merge_duplicate(existing: &Ticket, incoming: &Ticket) -> Ticket {
    assert_eq!(existing.fingerprint, incoming.fingerprint);
    Ticket {
        occurrences: existing.occurrences + incoming.occurrences,
        ..existing.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_existing_identity_and_v1_priority() {
        let older=Ticket{id:1,fingerprint:"mail-down".into(),priority:3,occurrences:1};
        let incoming=Ticket{id:2,priority:1,..older.clone()};
        let merged=merge_duplicate(&older,&incoming);
        assert_eq!(merged.id,1);assert_eq!(merged.priority,3);assert_eq!(merged.fingerprint,"mail-down");
    }
    #[test]
    #[should_panic]
    fn rejects_mismatching_fingerprints() {
        let a=Ticket{id:1,fingerprint:"mail-down".into(),priority:3,occurrences:1};
        let b=Ticket{fingerprint:"printer-down".into(),..a.clone()};merge_duplicate(&a,&b);
    }
    #[test]
    fn merges_occurrences() {
        let a = Ticket { id: 1, fingerprint: "mail-down".into(), priority: 3, occurrences: 1 };
        let b = Ticket { id: 2, occurrences: 2, ..a.clone() };
        assert_eq!(merge_duplicate(&a, &b).occurrences, 3);
    }
}
