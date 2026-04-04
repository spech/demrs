// ─────────────────────────────────────────────
// Freeze Frame Module
// ─────────────────────────────────────────────

use crate::event_config::SNAPSHOT_DATA_SIZE;

// ─────────────────────────────────────────────
// EventId
// ─────────────────────────────────────────────

/// Type alias for event identifiers.
pub type EventId = u16;

// ─────────────────────────────────────────────
// FreezeFrameListError
// ─────────────────────────────────────────────

/// Error type for [`FreezeFrameList`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeFrameListError {
    /// The freeze frames list has reached its maximum capacity.
    ListFullError,
}

// ─────────────────────────────────────────────
// FreezeFrame
// ─────────────────────────────────────────────

/// Persistent data associated with an [`Event`].
///
/// Stores event metadata that survives across power cycles, including:
/// - Event identifier
/// - Priority for ordering in [`FreezeFrameList`]
/// - Timestamps for first and last occurrence
/// - Snapshot data captured at the time of the event
///
/// ## Usage
///
/// [`FreezeFrame`] instances are stored in [`FreezeFrameList`]
/// and managed by [`EventManager`]. Snapshot data is automatically
/// captured by [`EventManager::auto_capture_snapshot()`].
#[derive(Clone)]
pub struct FreezeFrame {
    /// Unique identifier for this event.
    pub event_id: EventId,
    /// Priority value for ordering in [`FreezeFrameList`].
    pub priority: u8,
    /// Timestamp of the first occurrence (0 = not set).
    pub first_occurrence_time: u32,
    /// Timestamp of the last occurrence (0 = not set).
    pub last_occurrence_time: u32,
    /// Snapshot data captured at the time of the event.
    pub snapshot_data: [u8; SNAPSHOT_DATA_SIZE],
}

// ─────────────────────────────────────────────
// FreezeFrameList
// ─────────────────────────────────────────────

/// Result of finding the lowest priority entry in a [`FreezeFrameList`].
pub struct LowestPriorityResult {
    pub index: usize,
    pub priority: u8,
}

/// A fixed-capacity list of [`FreezeFrame`] entries.
///
/// Maintains [`FreezeFrame`] entries sorted by priority in ascending order.
/// Used for storing persistent event metadata that survives across power cycles.
///
/// ## Capacity
///
/// The list has a fixed capacity of 24 entries. Attempting to insert
/// when full returns [`FreezeFrameListError::ListFullError`].
///
/// ## Usage
///
/// Create an instance with [`FreezeFrameList::from_nvm()`], insert records
/// with [`FreezeFrameList::insert()`], and access them via
/// [`FreezeFrameList::get_by_event_id()`] or iterators.
///
/// For embedded systems with NVM, the `FreezeFrameList` should be stored
/// in NVM to persist across power cycles.
pub struct FreezeFrameList {
    data: [Option<FreezeFrame>; 24],
    len: usize,
}

impl FreezeFrameList {
    const CAPACITY: usize = 24;

    /// Returns a mutable reference to the list.
    ///
    /// # Returns
    ///
    /// A mutable reference to this `FreezeFrameList`.
    pub fn as_mut(&mut self) -> &mut FreezeFrameList {
        self
    }

    /// Creates a `FreezeFrameList` from NVM storage.
    ///
    /// # Arguments
    ///
    /// * `nvm_data` - Reference to NVM storage array
    ///
    /// # Returns
    ///
    /// A new list initialized from NVM data.
    pub const fn from_nvm(nvm_data: &'static [Option<FreezeFrame>; 24]) -> Self {
        let mut len = 0;
        let mut i = 0;
        while i < 24 {
            if nvm_data[i].is_some() {
                len += 1;
            }
            i += 1;
        }
        let mut data = [const { None }; 24];
        i = 0;
        while i < 24 {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    nvm_data.as_ptr().add(i),
                    (&mut data as *mut [Option<FreezeFrame>; 24] as *mut Option<FreezeFrame>)
                        .add(i),
                    1,
                );
            }
            i += 1;
        }
        Self { data, len }
    }

    /// Returns the number of entries in the list.
    ///
    /// # Returns
    ///
    /// The number of entries currently stored in the list.
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

    /// Clears all entries from the list.
    pub fn clear(&mut self) {
        self.data = [const { None }; 24];
        self.len = 0;
    }

    /// Returns a reference to the entry with the given event ID.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(&FreezeFrame)` if an entry with the given event ID exists, `None` otherwise.
    pub fn get_by_event_id(&self, event_id: EventId) -> Option<&FreezeFrame> {
        self.iter().find(|ff| ff.event_id == event_id)
    }

    /// Returns a mutable reference to the entry with the given event ID.
    ///
    /// # Arguments
    ///
    /// * `event_id` - The event ID to search for.
    ///
    /// # Returns
    ///
    /// `Some(&mut FreezeFrame)` if an entry with the given event ID exists, `None` otherwise.
    pub fn get_by_event_id_mut(&mut self, event_id: EventId) -> Option<&mut FreezeFrame> {
        self.iter_mut().find(|ff| ff.event_id == event_id)
    }

    /// Returns a mutable reference to the entry at the given index.
    ///
    /// # Arguments
    ///
    /// * `index` - The index of the entry to retrieve.
    ///
    /// # Returns
    ///
    /// `Some(&mut FreezeFrame)` if the index is valid, `None` otherwise.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut FreezeFrame> {
        if index < self.len {
            self.data[index].as_mut()
        } else {
            None
        }
    }

    /// Returns an iterator over all entries in insertion order.
    ///
    /// # Returns
    ///
    /// An iterator yielding references to entries.
    pub fn iter(&self) -> impl Iterator<Item = &FreezeFrame> {
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
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut FreezeFrame> {
        self.data
            .iter_mut()
            .take(self.len)
            .filter_map(|opt| opt.as_mut())
    }

    /// Inserts a new entry into the list, maintaining priority order.
    ///
    /// If the list is full, replaces the lowest priority entry if the new entry has higher priority.
    /// The `data` field is zero-initialized.
    ///
    /// # Arguments
    ///
    /// * `freeze_frame` - The entry to insert.
    ///
    /// # Returns
    ///
    /// `Ok(index)` - The index where the entry was inserted.
    /// `Err(FreezeFrameListError::ListFullError)` if the list is full and no entry has lower priority.
    pub fn insert(&mut self, mut freeze_frame: FreezeFrame) -> Result<usize, FreezeFrameListError> {
        freeze_frame.snapshot_data = [0u8; SNAPSHOT_DATA_SIZE];

        if self.is_full() {
            let new_priority = freeze_frame.priority;
            let lowest = self.find_lowest_priority();
            if new_priority < lowest.priority {
                self.remove(lowest.index);
            } else {
                return Err(FreezeFrameListError::ListFullError);
            }
        }

        let priority = freeze_frame.priority;

        let pos = self.data[..self.len]
            .iter()
            .position(|opt| opt.as_ref().map(|e| e.priority > priority).unwrap_or(false))
            .unwrap_or(self.len);

        for i in (pos..self.len).rev() {
            self.data[i + 1] = self.data[i].take();
        }

        self.data[pos] = Some(freeze_frame);
        self.len += 1;
        Ok(pos)
    }

    /// Finds the entry with the lowest priority.
    ///
    /// "Lowest priority" means the highest priority value (worst). When multiple entries
    /// have the same lowest priority, returns the oldest one based on `last_occurrence_time`.
    ///
    /// # Returns
    ///
    /// A struct containing the index and priority of the lowest priority entry.
    /// Returns index=0, priority=0 if the list is empty.
    pub fn find_lowest_priority(&self) -> LowestPriorityResult {
        if self.len == 0 {
            return LowestPriorityResult {
                index: 0,
                priority: u8::MIN,
            };
        }

        let mut lowest_idx = 0;
        let mut lowest_priority = u8::MIN;
        let mut oldest_timestamp = u32::MAX;

        for i in 0..self.len {
            if let Some(ff) = &self.data[i] {
                if ff.priority > lowest_priority {
                    lowest_priority = ff.priority;
                    lowest_idx = i;
                    oldest_timestamp = ff.last_occurrence_time;
                } else if ff.priority == lowest_priority {
                    if ff.last_occurrence_time < oldest_timestamp {
                        oldest_timestamp = ff.last_occurrence_time;
                        lowest_idx = i;
                    }
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
    /// `Some(FreezeFrame)` if the index is valid, `None` otherwise.
    pub fn remove(&mut self, index: usize) -> Option<FreezeFrame> {
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
    /// `Some(FreezeFrame)` if an entry with the given priority exists, `None` otherwise.
    pub fn remove_by_priority(&mut self, priority: u8) -> Option<FreezeFrame> {
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

    impl FreezeFrameList {
        fn test_new() -> Self {
            static mut NVM: [Option<FreezeFrame>; 24] = [const { None }; 24];
            unsafe { FreezeFrameList::from_nvm(&*(&raw const NVM)) }
        }
    }

    fn create_freeze_frame(event_id: u16, priority: u8, first: u32, last: u32) -> FreezeFrame {
        FreezeFrame {
            event_id,
            priority,
            first_occurrence_time: first,
            last_occurrence_time: last,
            snapshot_data: [0u8; SNAPSHOT_DATA_SIZE],
        }
    }

    #[test]
    fn fn_insert_full_returns_error() {
        let mut list = FreezeFrameList::test_new();

        for i in 0..24 {
            let freeze_frame = create_freeze_frame(i, 10 + i as u8, 0, 0);
            list.insert(freeze_frame).unwrap();
        }

        assert!(list.is_full());
        assert_eq!(list.len(), 24);

        let freeze_frame = create_freeze_frame(99, 100, 0, 0);
        let result = list.insert(freeze_frame);
        assert_eq!(result.unwrap_err(), FreezeFrameListError::ListFullError);

        assert_eq!(list.len(), 24);
        assert!(list.get_by_event_id(0).is_some());
        assert!(list.get_by_event_id(23).is_some());
        assert!(list.get_by_event_id(99).is_none());
    }

    #[test]
    fn fn_insert_full_evicts_lowest_when_new_has_higher_priority() {
        let mut list = FreezeFrameList::test_new();

        for i in 0..24 {
            let freeze_frame = create_freeze_frame(i, 10 + i as u8, 0, 0);
            list.insert(freeze_frame).unwrap();
        }

        let new_rec = create_freeze_frame(99, 5, 100, 200);
        let result = list.insert(new_rec);
        assert!(result.is_ok());
        assert_eq!(list.len(), 24);
        assert!(list.is_full());
        assert!(list.get_by_event_id(99).is_some());
    }

    #[test]
    fn fn_insert_returns_index() {
        let mut list = FreezeFrameList::test_new();

        let result = list.insert(create_freeze_frame(1, 10, 0, 0));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        let result = list.insert(create_freeze_frame(2, 5, 0, 0));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn fn_insert_zero_initializes_data() {
        let mut list = FreezeFrameList::test_new();

        let idx = list.insert(create_freeze_frame(1, 10, 100, 200)).unwrap();
        let ff = list.get_mut(idx).unwrap();

        assert_eq!(ff.snapshot_data, [0u8; SNAPSHOT_DATA_SIZE]);
    }

    #[test]
    fn fn_is_empty_return_true_when_list_is_empty() {
        let list = FreezeFrameList::test_new();
        assert!(list.is_empty());
    }

    #[test]
    fn fn_as_mut_returns_mutable_reference() {
        let mut list = FreezeFrameList::test_new();
        let _ = list.as_mut();
        assert!(list.is_empty());
    }

    #[test]
    fn fn_from_nvm_initializes_from_nvm_data() {
        static mut NVM_DATA: [Option<FreezeFrame>; 24] = [const { None }; 24];
        unsafe {
            NVM_DATA[0] = Some(FreezeFrame {
                event_id: 1,
                priority: 10,
                first_occurrence_time: 100,
                last_occurrence_time: 200,
                snapshot_data: [0u8; SNAPSHOT_DATA_SIZE],
            });
        }

        let list = unsafe { FreezeFrameList::from_nvm(&*(&raw const NVM_DATA)) };
        assert_eq!(list.len(), 1);
        assert!(list.get_by_event_id(1).is_some());
    }

    #[test]
    fn fn_remove_out_of_bounds_returns_none() {
        let mut list = FreezeFrameList::test_new();

        let result = list.remove(0);
        assert!(result.is_none());

        list.insert(create_freeze_frame(1, 5, 0, 0)).unwrap();

        let result = list.remove(5);
        assert!(result.is_none());

        let result = list.remove(100);
        assert!(result.is_none());
    }

    #[test]
    fn fn_remove_by_priority() {
        let mut list = FreezeFrameList::test_new();

        list.insert(create_freeze_frame(1, 5, 0, 0)).unwrap();
        list.insert(create_freeze_frame(2, 10, 0, 0)).unwrap();
        list.insert(create_freeze_frame(3, 15, 0, 0)).unwrap();

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
    fn fn_find_lowest_priority() {
        let mut list = FreezeFrameList::test_new();

        list.insert(create_freeze_frame(1, 10, 0, 0)).unwrap();
        list.insert(create_freeze_frame(2, 5, 0, 0)).unwrap();
        list.insert(create_freeze_frame(3, 15, 0, 0)).unwrap();

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 15);
        assert_eq!(result.index, 2);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 3);
    }

    #[test]
    fn fn_find_lowest_priority_returns_first_when_all_have_same_or_higher_priority() {
        let mut list = FreezeFrameList::test_new();

        list.insert(create_freeze_frame(1, 10, 0, 0)).unwrap();
        list.insert(create_freeze_frame(2, 20, 0, 0)).unwrap();

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 20);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 2);
    }

    #[test]
    fn fn_find_lowest_priority_returns_oldest_when_priorities_equal() {
        let mut list = FreezeFrameList::test_new();

        list.insert(create_freeze_frame(1, 10, 300, 300)).unwrap();
        list.insert(create_freeze_frame(2, 10, 100, 100)).unwrap();
        list.insert(create_freeze_frame(3, 10, 200, 200)).unwrap();

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 10);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 2);
    }

    #[test]
    fn fn_find_lowest_priority_do_nothing_branch_when_newer() {
        let mut list = FreezeFrameList::test_new();

        list.insert(create_freeze_frame(1, 10, 100, 100)).unwrap();
        list.insert(create_freeze_frame(2, 10, 200, 200)).unwrap();

        let result = list.find_lowest_priority();
        assert_eq!(result.priority, 10);
        assert_eq!(list.iter().nth(result.index).unwrap().event_id, 1);
    }

    #[test]
    fn fn_get_mut_returns_mutable_reference() {
        let mut list = FreezeFrameList::test_new();
        list.insert(create_freeze_frame(1, 10, 100, 200)).unwrap();

        let ff = list.get_mut(0).unwrap();
        assert_eq!(ff.event_id, 1);
        assert_eq!(ff.first_occurrence_time, 100);

        ff.last_occurrence_time = 300;
        let _ = ff;

        let ff = list.get_mut(0).unwrap();
        assert_eq!(ff.last_occurrence_time, 300);
    }

    #[test]
    fn fn_get_mut_returns_none_for_out_of_bounds() {
        let mut list = FreezeFrameList::test_new();
        assert!(list.get_mut(0).is_none());

        list.insert(create_freeze_frame(1, 10, 0, 0)).unwrap();
        assert!(list.get_mut(1).is_none());
    }
}
