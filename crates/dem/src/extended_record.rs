// ─────────────────────────────────────────────
// Extended Record Module
// ─────────────────────────────────────────────

// ─────────────────────────────────────────────
// EventId
// ─────────────────────────────────────────────

/// Type alias for event identifiers.
pub type EventId = u16;

// ─────────────────────────────────────────────
// EventManagerError
// ─────────────────────────────────────────────

/// Error type for [`EventManager`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventManagerError {
    /// The extended records list has reached its maximum capacity.
    ListFullError,
    /// The provided event ID is out of bounds.
    InvalidEventIdError,
    /// The event step operation failed.
    EventStepError,
    /// The EventManager is not initialized (state is Off).
    NotInitializedError,
}

// ─────────────────────────────────────────────
// ExtendedRecord
// ─────────────────────────────────────────────

/// Persistent data associated with an [`Event`].
///
/// Stores event metadata that survives across power cycles, including:
/// - Event identifier
/// - Priority for ordering in [`ExtendedRecordList`]
/// - Timestamps for first and last save operations
///
/// ## Usage
///
/// [`ExtendedRecord`] instances are stored in [`ExtendedRecordList`]
/// and managed by [`EventManager`].
#[derive(Clone)]
pub struct ExtendedRecord {
    /// Unique identifier for this event.
    pub event_id: EventId,
    /// Priority value for ordering in [`ExtendedRecordList`].
    pub priority: u8,
    /// Timestamp of the first save operation (0 = not set).
    pub date_at_first_save: u32,
    /// Timestamp of the last save operation (0 = not set).
    pub date_at_last_save: u32,
}

// ─────────────────────────────────────────────
// ExtendedRecordList
// ─────────────────────────────────────────────

/// Result of finding the lowest priority entry in an [`ExtendedRecordList`].
pub struct LowestPriorityResult {
    pub index: usize,
    pub priority: u8,
}

/// A fixed-capacity list of [`ExtendedRecord`] entries.
///
/// Maintains [`ExtendedRecord`] entries sorted by priority in ascending order.
/// Used for storing persistent event metadata that survives across power cycles.
///
/// ## Capacity
///
/// The list has a fixed capacity of 24 entries. Attempting to insert
/// when full returns [`EventManagerError::ListFullError`].
///
/// ## Usage
///
/// Create an instance with [`ExtendedRecordList::new()`], insert records
/// with [`ExtendedRecordList::insert()`], and access them via
/// [`ExtendedRecordList::get_by_event_id()`] or iterators.
pub struct ExtendedRecordList {
    data: [Option<ExtendedRecord>; 24],
    len: usize,
}

impl ExtendedRecordList {
    const CAPACITY: usize = 24;

    /// Creates a new, empty `ExtendedRecordList`.
    ///
    /// # Returns
    ///
    /// A new empty list with capacity for 24 entries.
    pub const fn new() -> Self {
        Self {
            data: [const { None }; 24],
            len: 0,
        }
    }

    pub fn as_mut(&mut self) -> &mut ExtendedRecordList {
        self
    }

    /// Returns the number of entries in the list.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the list contains no entries.
    ///
    /// # Returns
    ///
    /// `true` if the list is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if the list has reached its maximum capacity.
    ///
    /// # Returns
    ///
    /// `true` if the list is full (24 entries).
    pub fn is_full(&self) -> bool {
        self.len == Self::CAPACITY
    }

    /// Returns a reference to the entry with the given event ID.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(&ExtendedRecord)` if an entry with the given event ID exists, `None` otherwise.
    pub fn get_by_event_id(&self, event_id: EventId) -> Option<&ExtendedRecord> {
        self.iter().find(|ext_rec| ext_rec.event_id == event_id)
    }

    /// Returns a mutable reference to the entry with the given event ID.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(&mut ExtendedRecord)` if an entry with the given event ID exists, `None` otherwise.
    pub fn get_by_event_id_mut(&mut self, event_id: EventId) -> Option<&mut ExtendedRecord> {
        self.iter_mut().find(|ext_rec| ext_rec.event_id == event_id)
    }

    /// Returns an iterator over all entries in insertion order.
    ///
    /// # Returns
    ///
    /// An iterator yielding references to entries.
    pub fn iter(&self) -> impl Iterator<Item = &ExtendedRecord> {
        self.data
            .iter()
            .take(self.len)
            .filter_map(|opt| opt.as_ref())
    }

    /// Returns a mutable iterator over all entries in insertion order.
    ///
    /// # Returns
    ///
    /// An iterator yielding mutable references to entries.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ExtendedRecord> {
        self.data
            .iter_mut()
            .take(self.len)
            .filter_map(|opt| opt.as_mut())
    }

    /// Inserts a new entry into the list, maintaining priority order.
    ///
    /// If the list is full, replaces the lowest priority entry if the new entry has higher priority.
    ///
    /// # Arguments
    ///
    /// * `ext_rec` - The entry to insert.
    ///
    /// # Returns
    ///
    /// `Ok(())` if insertion succeeded.
    /// `Err(EventManagerError::ListFullError)` if the list is full and no entry has lower priority.
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

    /// Finds the entry with the lowest priority.
    ///
    /// # Returns
    ///
    /// A struct containing the index and priority of the lowest priority entry.
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

    /// Removes and returns the entry at the given index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the entry to remove.
    ///
    /// # Returns
    ///
    /// `Some(ExtendedRecord)` if the index is valid, `None` otherwise.
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

    /// Removes and returns the entry with the given priority.
    ///
    /// # Arguments
    ///
    /// * `priority` - The priority of the entry to remove.
    ///
    /// # Returns
    ///
    /// `Some(ExtendedRecord)` if an entry with the given priority exists, `None` otherwise.
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
                date_at_first_save: 0,
                date_at_last_save: 0,
            };
            list.insert(ext_rec).unwrap();
        }

        assert!(list.is_full());
        assert_eq!(list.len(), 24);

        let ext_rec = ExtendedRecord {
            event_id: 99,
            priority: 100,
            date_at_first_save: 0,
            date_at_last_save: 0,
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
            date_at_first_save: 0,
            date_at_last_save: 0,
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

        for (i, &priority) in [5, 10, 15].iter().enumerate() {
            let ext_rec = ExtendedRecord {
                event_id: (i + 1) as EventId,
                priority,
                date_at_first_save: 0,
                date_at_last_save: 0,
            };
            list.insert(ext_rec).unwrap();
        }

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

        for (i, &priority) in [10, 5, 15].iter().enumerate() {
            let ext_rec = ExtendedRecord {
                event_id: (i + 1) as EventId,
                priority,
                date_at_first_save: 0,
                date_at_last_save: 0,
            };
            list.insert(ext_rec).unwrap();
        }

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 5);
        assert_eq!(result.index, 0);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 2);
    }
}
