pub type EventId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventManagerError {
    ListFullError,
    InvalidEventIdError,
    EventStepError,
}

#[derive(Clone)]
pub struct ExtendedRecord {
    pub event_id: EventId,
    pub priority: u8,
    pub date_at_first_save: Option<chrono::DateTime<chrono::Utc>>,
    pub date_at_last_save: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct LowestPriorityResult {
    pub index: usize,
    pub priority: u8,
}

pub struct ExtendedRecordList {
    data: [Option<ExtendedRecord>; 24],
    len: usize,
}

impl ExtendedRecordList {
    const CAPACITY: usize = 24;

    pub const fn new() -> Self {
        Self {
            data: [const { None }; 24],
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn is_full(&self) -> bool {
        self.len == Self::CAPACITY
    }

    pub fn get_by_event_id(&self, event_id: EventId) -> Option<&ExtendedRecord> {
        self.iter().find(|ext_rec| ext_rec.event_id == event_id)
    }

    pub fn get_by_event_id_mut(&mut self, event_id: EventId) -> Option<&mut ExtendedRecord> {
        self.iter_mut().find(|ext_rec| ext_rec.event_id == event_id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ExtendedRecord> {
        self.data
            .iter()
            .take(self.len)
            .filter_map(|opt| opt.as_ref())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ExtendedRecord> {
        self.data
            .iter_mut()
            .take(self.len)
            .filter_map(|opt| opt.as_mut())
    }

    pub fn insert(&mut self, ext_rec: ExtendedRecord) -> Result<(), EventManagerError> {
        if self.is_full() {
            let new_priority = ext_rec.priority;
            let lowest = self.find_lowest_priority();
            if new_priority < lowest.priority {
                self.remove(lowest.index);
            } else {
                return Err(EventManagerError::ListFullError);
            }
        }

        let priority = ext_rec.priority;

        let pos = self.data[..self.len]
            .iter()
            .position(|opt| opt.as_ref().map(|e| e.priority > priority).unwrap_or(false))
            .unwrap_or(self.len);

        for i in (pos..self.len).rev() {
            self.data[i + 1] = self.data[i].take();
        }

        self.data[pos] = Some(ext_rec);
        self.len += 1;
        Ok(())
    }

    pub fn find_lowest_priority(&self) -> LowestPriorityResult {
        let mut lowest_idx = 0;
        let mut lowest_priority = u8::MAX;

        for i in 0..self.len {
            if let Some(ext_rec) = &self.data[i] {
                if ext_rec.priority < lowest_priority {
                    lowest_priority = ext_rec.priority;
                    lowest_idx = i;
                }
            }
        }

        LowestPriorityResult {
            index: lowest_idx,
            priority: lowest_priority,
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<ExtendedRecord> {
        if index >= self.len {
            return None;
        }

        let removed = self.data[index].take();

        for i in index..self.len - 1 {
            self.data[i] = self.data[i + 1].take();
        }

        self.len -= 1;
        removed
    }

    pub fn remove_by_priority(&mut self, priority: u8) -> Option<ExtendedRecord> {
        let index = self.data[..self.len].iter().position(|opt| {
            opt.as_ref()
                .map(|e| e.priority == priority)
                .unwrap_or(false)
        });

        if let Some(idx) = index {
            self.remove(idx)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_record_list_insert_full_returns_error() {
        let mut list = ExtendedRecordList::new();

        for i in 0..24 {
            let ext_rec = ExtendedRecord {
                event_id: i,
                priority: 10 + i as u8,
                date_at_first_save: None,
                date_at_last_save: None,
            };
            list.insert(ext_rec).unwrap();
        }

        assert!(list.is_full());
        assert_eq!(list.len(), 24);

        let ext_rec = ExtendedRecord {
            event_id: 99,
            priority: 100,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let result = list.insert(ext_rec);
        assert_eq!(result.unwrap_err(), EventManagerError::ListFullError);

        assert_eq!(list.len(), 24);
        assert!(list.get_by_event_id(0).is_some());
        assert!(list.get_by_event_id(23).is_some());
        assert!(list.get_by_event_id(99).is_none());
    }

    #[test]
    fn extended_record_list_remove_out_of_bounds_returns_none() {
        let mut list = ExtendedRecordList::new();

        let result = list.remove(0);
        assert!(result.is_none());

        let ext_rec = ExtendedRecord {
            event_id: 1,
            priority: 5,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        list.insert(ext_rec).unwrap();

        let result = list.remove(5);
        assert!(result.is_none());

        let result = list.remove(100);
        assert!(result.is_none());
    }

    #[test]
    fn extended_record_list_remove_by_priority() {
        let mut list = ExtendedRecordList::new();

        let ext_rec1 = ExtendedRecord {
            event_id: 1,
            priority: 5,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec2 = ExtendedRecord {
            event_id: 2,
            priority: 10,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec3 = ExtendedRecord {
            event_id: 3,
            priority: 15,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        list.insert(ext_rec1).unwrap();
        list.insert(ext_rec2).unwrap();
        list.insert(ext_rec3).unwrap();

        assert_eq!(list.len(), 3);

        let removed = list.remove_by_priority(10).unwrap();
        assert_eq!(removed.event_id, 2);
        assert_eq!(list.len(), 2);
        assert!(list.get_by_event_id(2).is_none());
        assert!(list.get_by_event_id(1).is_some());
        assert!(list.get_by_event_id(3).is_some());

        let result = list.remove_by_priority(200);
        assert!(result.is_none());
    }

    #[test]
    fn extended_record_list_find_lowest_priority() {
        let mut list = ExtendedRecordList::new();

        let ext_rec1 = ExtendedRecord {
            event_id: 1,
            priority: 10,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec2 = ExtendedRecord {
            event_id: 2,
            priority: 5,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        let ext_rec3 = ExtendedRecord {
            event_id: 3,
            priority: 15,
            date_at_first_save: None,
            date_at_last_save: None,
        };
        list.insert(ext_rec1).unwrap();
        list.insert(ext_rec2).unwrap();
        list.insert(ext_rec3).unwrap();

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 5);
        assert_eq!(result.index, 0);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 2);
    }
}
