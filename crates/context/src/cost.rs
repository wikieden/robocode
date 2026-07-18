use std::collections::BTreeSet;

use viden_types::{CostAmount, CostLedgerTotals, CostScope, CostUsageRecord};

#[derive(Debug, Clone, Default)]
pub struct CostLedger {
    records: Vec<CostUsageRecord>,
    usage_ids: BTreeSet<String>,
}

impl CostLedger {
    pub fn append(&mut self, record: CostUsageRecord) -> bool {
        if !self.usage_ids.insert(record.usage_id.clone()) {
            return false;
        }
        self.records.push(record);
        true
    }

    pub fn records(&self) -> &[CostUsageRecord] {
        &self.records
    }

    pub fn records_for_scope(&self, scope: &CostScope) -> Vec<&CostUsageRecord> {
        self.records
            .iter()
            .filter(|record| record.scopes.iter().any(|candidate| candidate == scope))
            .collect()
    }

    pub fn totals_for_scope(&self, scope: &CostScope) -> CostLedgerTotals {
        totals_from_records(self.records_for_scope(scope))
    }
}

pub fn totals_from_records<'a>(
    records: impl IntoIterator<Item = &'a CostUsageRecord>,
) -> CostLedgerTotals {
    let mut totals = CostLedgerTotals::default();
    let mut saw_unknown_actual = false;
    let mut saw_actual = false;

    for record in records {
        totals.record(record);
        if record.actual_cost.is_some() {
            saw_actual = true;
        } else {
            saw_unknown_actual = true;
        }
    }

    if saw_unknown_actual || !saw_actual {
        totals.actual_cost = None;
        totals.total_actual_cost_micro_usd = None;
    }
    totals
}

pub fn add_amount(current: &mut Option<CostAmount>, amount: &CostAmount) {
    match current {
        Some(current) => {
            current.micro_units = current.micro_units.saturating_add(amount.micro_units);
        }
        None => *current = Some(amount.clone()),
    }
}
