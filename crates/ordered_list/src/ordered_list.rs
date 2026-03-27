//! # Ordered List
//!
//! A priority-ordered list implementation with fixed capacity.
//!
//! ## Overview
//!
//! This module provides three ordered list variants:
//! - [`OrderedList`] - owns its elements
//! - [`OrderedListRef`] - holds references to elements
//! - [`OrderedListIdx`] - holds indices/handles to elements
//!
//! All variants maintain elements sorted by priority (ascending), allowing O(log n)
//! insertion via binary search.
//!
//! ## Usage
//!
//! ```rust
//! use ordered_list::HasPriority;
//!
//! struct Task {
//!     priority: u8,
//!     name: &'static str,
//! }
//!
//! impl HasPriority for Task {
//!     fn priority(&self) -> u8 {
//!         self.priority
//!     }
//! }
//!
//! let mut list = ordered_list::OrderedList::<Task>::new(10);
//! list.insert(Task { priority: 2, name: "medium" });
//! list.insert(Task { priority: 1, name: "high" });
//! list.insert(Task { priority: 3, name: "low" });
//!
//! assert_eq!(list.peek_lowest().unwrap().name, "high");
//! assert_eq!(list.peek_highest().unwrap().name, "low");
//! ```
//!
//! ## Features
//!
//! - Fixed capacity with zero allocations after initialization
//! - Binary search insertion: O(log n)
//! - Pop from either end: O(1)
//! - Range queries and draining operations
//! - Iteration support (by-ref, by-mut, by-value)
//!
//! ## Priority Handling
//!
//! When multiple elements share the same priority, insertion order is preserved
//! (elements are inserted after existing elements with the same priority).

// ─────────────────────────────────────────────
// HasPriority Trait
// ─────────────────────────────────────────────

/// Trait for types that have a priority value.
///
/// Types implementing this trait can be stored in any of the ordered list variants.
///
/// # Example
///
/// ```
/// use ordered_list::HasPriority;
///
/// struct Event {
///     priority: u8,
///     data: u32,
/// }
///
/// impl HasPriority for Event {
///     fn priority(&self) -> u8 {
///         self.priority
///     }
/// }
/// ```
pub trait HasPriority {
    /// Returns the priority value of this element.
    ///
    /// Higher values indicate higher priority (processed later in a highest-first workflow).
    fn priority(&self) -> u8;
}

// ─────────────────────────────────────────────
// Error
// ─────────────────────────────────────────────

/// Error type for [`OrderedList`] operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderedListError {
    /// The list has reached its capacity and cannot accept more elements.
    ListFull,
}

// ─────────────────────────────────────────────
// OrderedList - Owned Elements
// ─────────────────────────────────────────────

/// An ordered list that owns its elements.
///
/// Maintains elements sorted by priority in ascending order.
/// Uses a pre-allocated buffer with fixed capacity.
///
/// # Type Parameters
///
/// - `T`: Element type that implements [`HasPriority`]
///
/// # Example
///
/// ```
/// use ordered_list::{HasPriority, OrderedList};
///
/// #[derive(Debug)]
/// struct Item(u8, &'static str);
///
/// impl HasPriority for Item {
///     fn priority(&self) -> u8 { self.0 }
/// }
///
/// let mut list = OrderedList::<Item>::new(3);
/// list.insert(Item(1, "a")).unwrap();
/// list.insert(Item(3, "c")).unwrap();
/// list.insert(Item(2, "b")).unwrap();
///
/// assert_eq!(list.pop_lowest().unwrap().1, "a");
/// assert_eq!(list.pop_highest().unwrap().1, "c");
/// ```
///
/// # Capacity
///
/// The list is created with a fixed capacity. Insertion returns `Err(item)` when full.
pub struct OrderedList<T: HasPriority> {
    items: Vec<T>,
}

impl<T: HasPriority> OrderedList<T> {
    /// Creates a new empty `OrderedList` with the specified capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Maximum number of elements the list can hold
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList};
    ///
    /// #[derive(Debug)]
    /// struct Task { priority: u8 }
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.priority } }
    ///
    /// let list: OrderedList<Task> = OrderedList::new(100);
    /// assert!(list.is_empty());
    /// assert_eq!(list.capacity(), 100);
    /// ```
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    /// Returns the maximum number of elements this list can hold.
    pub fn capacity(&self) -> usize {
        self.items.capacity()
    }

    /// Returns the current number of elements in the list.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the list contains no elements.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns `true` if the list has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() == self.items.capacity()
    }

    /// Inserts an element into the list, maintaining priority order.
    ///
    /// Elements are sorted in ascending priority order.
    /// If multiple elements share the same priority, the new element is inserted after them.
    ///
    /// # Arguments
    ///
    /// * `item` - The element to insert
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the element was inserted successfully
    /// - `Err(OrderedListError::ListFull)` if the list is full
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList, OrderedListError};
    ///
    /// #[derive(Debug)]
    /// struct Task(u8);
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.0 } }
    ///
    /// let mut list = OrderedList::<Task>::new(2);
    /// assert!(list.insert(Task(1)).is_ok());
    /// assert!(list.insert(Task(2)).is_ok());
    /// assert_eq!(list.insert(Task(3)), Err(OrderedListError::ListFull)); // Full
    /// ```
    pub fn insert(&mut self, item: T) -> Result<(), OrderedListError> {
        if self.is_full() {
            return Err(OrderedListError::ListFull);
        }

        let priority = item.priority();
        let idx = match self.items.binary_search_by_key(&priority, |x| x.priority()) {
            Ok(idx) => {
                let insert_pos = self.items[idx..]
                    .iter()
                    .position(|x| x.priority() != priority)
                    .map(|pos| idx + pos)
                    .unwrap_or(self.items.len());
                insert_pos
            }
            Err(idx) => idx,
        };

        self.items.insert(idx, item);
        Ok(())
    }

    /// Removes and returns the element with the highest priority.
    ///
    /// Returns `None` if the list is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList};
    ///
    /// #[derive(Debug)]
    /// struct Task(u8);
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.0 } }
    ///
    /// let mut list = OrderedList::<Task>::new(10);
    /// list.insert(Task(1)).unwrap();
    /// list.insert(Task(3)).unwrap();
    /// list.insert(Task(2)).unwrap();
    ///
    /// assert_eq!(list.pop_highest().unwrap().0, 3);
    /// ```
    pub fn pop_highest(&mut self) -> Option<T> {
        self.items.pop()
    }

    /// Removes and returns the element with the lowest priority.
    ///
    /// Returns `None` if the list is empty.
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList};
    ///
    /// #[derive(Debug)]
    /// struct Task(u8);
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.0 } }
    ///
    /// let mut list = OrderedList::<Task>::new(10);
    /// list.insert(Task(1)).unwrap();
    /// list.insert(Task(3)).unwrap();
    /// list.insert(Task(2)).unwrap();
    ///
    /// assert_eq!(list.pop_lowest().unwrap().0, 1);
    /// ```
    pub fn pop_lowest(&mut self) -> Option<T> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0))
    }

    /// Returns a reference to the element with the highest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn peek_highest(&self) -> Option<&T> {
        self.items.last()
    }

    /// Returns a reference to the element with the lowest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn peek_lowest(&self) -> Option<&T> {
        self.items.first()
    }

    /// Returns an iterator over the list by reference.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Returns a mutable iterator over the list.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut()
    }

    /// Returns an iterator that consumes the list.
    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.items.into_iter()
    }

    /// Removes all elements from the list.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns a reference to the first element with the given priority.
    ///
    /// Returns `None` if no element with that priority exists.
    pub fn get_by_priority(&self, priority: u8) -> Option<&T> {
        let idx = self.items.binary_search_by_key(&priority, |x| x.priority()).ok()?;
        self.items.get(idx)
    }

    /// Returns an iterator over elements within the specified priority range.
    ///
    /// # Arguments
    ///
    /// * `lower` - Lower bound (inclusive)
    /// * `upper` - Upper bound (exclusive)
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList};
    ///
    /// #[derive(Debug)]
    /// struct Task(u8);
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.0 } }
    ///
    /// let mut list = OrderedList::<Task>::new(10);
    /// list.insert(Task(1)).unwrap();
    /// list.insert(Task(2)).unwrap();
    /// list.insert(Task(3)).unwrap();
    /// list.insert(Task(4)).unwrap();
    ///
    /// let mid: Vec<_> = list.range(2, 4).collect();
    /// assert_eq!(mid.len(), 2);
    /// ```
    pub fn range(&self, lower: u8, upper: u8) -> impl Iterator<Item = &T> {
        self.items.iter().filter(move |x| {
            let p = x.priority();
            p >= lower && p < upper
        })
    }

    /// Removes and returns all elements with priority greater than the given value.
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList};
    ///
    /// #[derive(Debug)]
    /// struct Task(u8);
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.0 } }
    ///
    /// let mut list = OrderedList::<Task>::new(10);
    /// list.insert(Task(1)).unwrap();
    /// list.insert(Task(2)).unwrap();
    /// list.insert(Task(3)).unwrap();
    ///
    /// let drained: Vec<_> = list.drain_above(2).collect();
    /// assert_eq!(drained.len(), 1);
    /// assert_eq!(drained[0].0, 3);
    /// ```
    pub fn drain_above(&mut self, priority: u8) -> impl Iterator<Item = T> + '_ {
        let split_idx = self
            .items
            .binary_search_by_key(&priority, |x| x.priority())
            .map(|i| i + 1)
            .unwrap_or_else(|i| i);

        self.items.drain(split_idx..)
    }

    /// Removes and returns all elements with priority less than the given value.
    ///
    /// # Example
    ///
    /// ```
    /// use ordered_list::{HasPriority, OrderedList};
    ///
    /// #[derive(Debug)]
    /// struct Task(u8);
    /// impl HasPriority for Task { fn priority(&self) -> u8 { self.0 } }
    ///
    /// let mut list = OrderedList::<Task>::new(10);
    /// list.insert(Task(1)).unwrap();
    /// list.insert(Task(2)).unwrap();
    /// list.insert(Task(3)).unwrap();
    ///
    /// let drained: Vec<_> = list.drain_below(2).collect();
    /// assert_eq!(drained.len(), 1);
    /// ```
    pub fn drain_below(&mut self, priority: u8) -> impl Iterator<Item = T> + '_ {
        let split_idx = self
            .items
            .binary_search_by_key(&priority, |x| x.priority())
            .unwrap_or_else(|i| i);

        self.items.drain(..split_idx)
    }
}

// ─────────────────────────────────────────────
// OrderedListRef - Borrowed Elements
// ─────────────────────────────────────────────

/// An ordered list that holds references to elements.
///
/// Useful when elements are owned elsewhere and you want a sorted view into them.
/// The list does not own the elements; it only holds references.
///
/// # Type Parameters
///
/// - `'a`: Lifetime of the references
/// - `T`: Element type that implements [`HasPriority`] and `'a`
///
/// # Example
///
/// ```
/// use ordered_list::{HasPriority, OrderedListRef};
///
/// #[derive(Debug)]
/// struct Task { priority: u8, name: &'static str }
/// impl HasPriority for Task { fn priority(&self) -> u8 { self.priority } }
///
/// let tasks = [
///     Task { priority: 2, name: "two" },
///     Task { priority: 1, name: "one" },
///     Task { priority: 3, name: "three" },
/// ];
///
/// let mut list = OrderedListRef::<Task>::new(3);
/// for task in &tasks {
///     list.insert(task).unwrap();
/// }
///
/// assert_eq!(list.peek_lowest().unwrap().name, "one");
/// assert_eq!(list.pop_highest().unwrap().name, "three");
/// ```
pub struct OrderedListRef<'a, T: HasPriority + 'a> {
    items: Vec<&'a T>,
}

impl<'a, T: HasPriority + 'a> OrderedListRef<'a, T> {
    /// Creates a new empty `OrderedListRef` with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    /// Returns the maximum number of references this list can hold.
    pub fn capacity(&self) -> usize {
        self.items.capacity()
    }

    /// Returns the current number of references in the list.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the list contains no references.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns `true` if the list has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() == self.items.capacity()
    }

    /// Inserts a reference to an element, maintaining priority order.
    ///
    /// # Arguments
    ///
    /// * `item` - A reference to the element to insert
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the reference was inserted successfully
    /// - `Err(OrderedListError::ListFull)` if the list is full
    pub fn insert(&mut self, item: &'a T) -> Result<(), OrderedListError> {
        if self.is_full() {
            return Err(OrderedListError::ListFull);
        }

        let priority = item.priority();
        let idx = match self.items.binary_search_by_key(&priority, |x| x.priority()) {
            Ok(idx) => {
                let insert_pos = self.items[idx..]
                    .iter()
                    .position(|x| x.priority() != priority)
                    .map(|pos| idx + pos)
                    .unwrap_or(self.items.len());
                insert_pos
            }
            Err(idx) => idx,
        };

        self.items.insert(idx, item);
        Ok(())
    }

    /// Removes and returns the reference with the highest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn pop_highest(&mut self) -> Option<&'a T> {
        self.items.pop()
    }

    /// Removes and returns the reference with the lowest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn pop_lowest(&mut self) -> Option<&'a T> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0))
    }

    /// Returns a reference to the element with the highest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn peek_highest(&self) -> Option<&&'a T> {
        self.items.last()
    }

    /// Returns a reference to the element with the lowest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn peek_lowest(&self) -> Option<&&'a T> {
        self.items.first()
    }

    /// Returns an iterator over the references.
    pub fn iter(&self) -> impl Iterator<Item = &&'a T> {
        self.items.iter()
    }

    /// Removes all references from the list.
    ///
    /// Note: This does not affect the original elements.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns a reference to the first element with the given priority.
    ///
    /// Returns `None` if no element with that priority exists.
    pub fn get_by_priority(&self, priority: u8) -> Option<&&'a T> {
        let idx = self.items.binary_search_by_key(&priority, |x| x.priority()).ok()?;
        self.items.get(idx)
    }

    /// Returns an iterator over elements within the specified priority range.
    ///
    /// # Arguments
    ///
    /// * `lower` - Lower bound (inclusive)
    /// * `upper` - Upper bound (exclusive)
    pub fn range(&self, lower: u8, upper: u8) -> impl Iterator<Item = &&'a T> {
        self.items.iter().filter(move |x| {
            let p = x.priority();
            p >= lower && p < upper
        })
    }
}

// ─────────────────────────────────────────────
// OrderedListIdx - Index/Handle Based
// ─────────────────────────────────────────────

/// An ordered list that holds indices or handles to elements.
///
/// Useful when elements are stored elsewhere (e.g., in a ring buffer) and you want
/// a sorted view by storing indices rather than full elements.
///
/// # Type Parameters
///
/// - `T`: Index/handle type that implements [`HasPriority`]
///
/// # Example
///
/// ```
/// use ordered_list::{HasPriority, OrderedListIdx};
///
/// #[derive(Debug)]
/// struct EventHandle(u8);  // Index into an event storage
/// impl HasPriority for EventHandle { fn priority(&self) -> u8 { self.0 } }
///
/// let mut list = OrderedListIdx::<EventHandle>::new(10);
/// list.insert(EventHandle(2)).unwrap();
/// list.insert(EventHandle(1)).unwrap();
/// list.insert(EventHandle(3)).unwrap();
///
/// assert_eq!(list.pop_lowest().unwrap().0, 1);
/// assert_eq!(list.pop_highest().unwrap().0, 3);
/// ```
pub struct OrderedListIdx<T: HasPriority> {
    items: Vec<T>,
}

impl<T: HasPriority> OrderedListIdx<T> {
    /// Creates a new empty `OrderedListIdx` with the specified capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            items: Vec::with_capacity(capacity),
        }
    }

    /// Returns the maximum number of indices this list can hold.
    pub fn capacity(&self) -> usize {
        self.items.capacity()
    }

    /// Returns the current number of indices in the list.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Returns `true` if the list contains no indices.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns `true` if the list has reached its capacity.
    pub fn is_full(&self) -> bool {
        self.items.len() == self.items.capacity()
    }

    /// Inserts an index/handle, maintaining priority order.
    ///
    /// # Arguments
    ///
    /// * `item` - The index/handle to insert
    ///
    /// # Returns
    ///
    /// - `Ok(())` if the index was inserted successfully
    /// - `Err(OrderedListError::ListFull)` if the list is full
    pub fn insert(&mut self, item: T) -> Result<(), OrderedListError> {
        if self.is_full() {
            return Err(OrderedListError::ListFull);
        }

        let priority = item.priority();
        let idx = match self.items.binary_search_by_key(&priority, |x| x.priority()) {
            Ok(idx) => {
                let insert_pos = self.items[idx..]
                    .iter()
                    .position(|x| x.priority() != priority)
                    .map(|pos| idx + pos)
                    .unwrap_or(self.items.len());
                insert_pos
            }
            Err(idx) => idx,
        };

        self.items.insert(idx, item);
        Ok(())
    }

    /// Removes and returns the index/handle with the highest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn pop_highest(&mut self) -> Option<T> {
        self.items.pop()
    }

    /// Removes and returns the index/handle with the lowest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn pop_lowest(&mut self) -> Option<T> {
        if self.items.is_empty() {
            return None;
        }
        Some(self.items.remove(0))
    }

    /// Returns a reference to the index/handle with the highest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn peek_highest(&self) -> Option<&T> {
        self.items.last()
    }

    /// Returns a reference to the index/handle with the lowest priority.
    ///
    /// Returns `None` if the list is empty.
    pub fn peek_lowest(&self) -> Option<&T> {
        self.items.first()
    }

    /// Returns an iterator over the indices/handles by reference.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter()
    }

    /// Returns a mutable iterator over the indices/handles.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.items.iter_mut()
    }

    /// Returns an iterator that consumes the list.
    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.items.into_iter()
    }

    /// Removes all indices/handles from the list.
    pub fn clear(&mut self) {
        self.items.clear();
    }

    /// Returns a reference to the first index/handle with the given priority.
    ///
    /// Returns `None` if no index with that priority exists.
    pub fn get_by_priority(&self, priority: u8) -> Option<&T> {
        let idx = self.items.binary_search_by_key(&priority, |x| x.priority()).ok()?;
        self.items.get(idx)
    }

    /// Returns an iterator over indices/handles within the specified priority range.
    ///
    /// # Arguments
    ///
    /// * `lower` - Lower bound (inclusive)
    /// * `upper` - Upper bound (exclusive)
    pub fn range(&self, lower: u8, upper: u8) -> impl Iterator<Item = &T> {
        self.items.iter().filter(move |x| {
            let p = x.priority();
            p >= lower && p < upper
        })
    }

    /// Removes and returns all indices/handles with priority greater than the given value.
    pub fn drain_above(&mut self, priority: u8) -> impl Iterator<Item = T> + '_ {
        let split_idx = self
            .items
            .binary_search_by_key(&priority, |x| x.priority())
            .map(|i| i + 1)
            .unwrap_or_else(|i| i);

        self.items.drain(split_idx..)
    }

    /// Removes and returns all indices/handles with priority less than or equal to the given value.
    pub fn drain_below(&mut self, priority: u8) -> impl Iterator<Item = T> + '_ {
        let split_idx = self
            .items
            .binary_search_by_key(&priority, |x| x.priority())
            .unwrap_or_else(|i| i);

        self.items.drain(..split_idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestEvent {
        priority: u8,
        id: u32,
    }

    impl HasPriority for TestEvent {
        fn priority(&self) -> u8 {
            self.priority
        }
    }

    #[test]
    fn ordered_list_insert_ascending() {
        let mut list = OrderedList::<TestEvent>::new(10);
        assert!(list.insert(TestEvent { priority: 1, id: 1 }).is_ok());
        assert!(list.insert(TestEvent { priority: 2, id: 2 }).is_ok());
        assert!(list.insert(TestEvent { priority: 3, id: 3 }).is_ok());

        assert_eq!(list.len(), 3);
        assert_eq!(list.peek_lowest().unwrap().id, 1);
        assert_eq!(list.peek_highest().unwrap().id, 3);
    }

    #[test]
    fn ordered_list_insert_descending() {
        let mut list = OrderedList::<TestEvent>::new(10);
        assert!(list.insert(TestEvent { priority: 3, id: 3 }).is_ok());
        assert!(list.insert(TestEvent { priority: 2, id: 2 }).is_ok());
        assert!(list.insert(TestEvent { priority: 1, id: 1 }).is_ok());

        assert_eq!(list.len(), 3);
        assert_eq!(list.peek_lowest().unwrap().id, 1);
        assert_eq!(list.peek_highest().unwrap().id, 3);
    }

    #[test]
    fn ordered_list_insert_duplicate_priority() {
        let mut list = OrderedList::<TestEvent>::new(10);
        assert!(list.insert(TestEvent { priority: 1, id: 1 }).is_ok());
        assert!(list.insert(TestEvent { priority: 1, id: 2 }).is_ok());
        assert!(list.insert(TestEvent { priority: 1, id: 3 }).is_ok());

        assert_eq!(list.len(), 3);
        let items: Vec<_> = list.iter().collect();
        assert_eq!(items[0].id, 1);
        assert_eq!(items[1].id, 2);
        assert_eq!(items[2].id, 3);
    }

    #[test]
    fn ordered_list_insert_full() {
        let mut list = OrderedList::<TestEvent>::new(2);
        assert!(list.insert(TestEvent { priority: 1, id: 1 }).is_ok());
        assert!(list.insert(TestEvent { priority: 2, id: 2 }).is_ok());
        assert_eq!(list.insert(TestEvent { priority: 3, id: 3 }), Err(OrderedListError::ListFull));
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn ordered_list_pop_highest() {
        let mut list = OrderedList::<TestEvent>::new(10);
        list.insert(TestEvent { priority: 1, id: 1 }).unwrap();
        list.insert(TestEvent { priority: 2, id: 2 }).unwrap();
        list.insert(TestEvent { priority: 3, id: 3 }).unwrap();

        assert_eq!(list.pop_highest().unwrap().id, 3);
        assert_eq!(list.pop_highest().unwrap().id, 2);
        assert_eq!(list.pop_highest().unwrap().id, 1);
        assert!(list.pop_highest().is_none());
    }

    #[test]
    fn ordered_list_pop_lowest() {
        let mut list = OrderedList::<TestEvent>::new(10);
        list.insert(TestEvent { priority: 1, id: 1 }).unwrap();
        list.insert(TestEvent { priority: 2, id: 2 }).unwrap();
        list.insert(TestEvent { priority: 3, id: 3 }).unwrap();

        assert_eq!(list.pop_lowest().unwrap().id, 1);
        assert_eq!(list.pop_lowest().unwrap().id, 2);
        assert_eq!(list.pop_lowest().unwrap().id, 3);
        assert!(list.pop_lowest().is_none());
    }

    #[test]
    fn ordered_list_get_by_priority() {
        let mut list = OrderedList::<TestEvent>::new(10);
        list.insert(TestEvent { priority: 1, id: 1 }).unwrap();
        list.insert(TestEvent { priority: 2, id: 2 }).unwrap();
        list.insert(TestEvent { priority: 3, id: 3 }).unwrap();

        assert_eq!(list.get_by_priority(2).unwrap().id, 2);
        assert!(list.get_by_priority(5).is_none());
    }

    #[test]
    fn ordered_list_drain() {
        let mut list = OrderedList::<TestEvent>::new(10);
        list.insert(TestEvent { priority: 1, id: 1 }).unwrap();
        list.insert(TestEvent { priority: 2, id: 2 }).unwrap();
        list.insert(TestEvent { priority: 3, id: 3 }).unwrap();
        list.insert(TestEvent { priority: 4, id: 4 }).unwrap();

        let drained: Vec<_> = list.drain_above(2).collect();
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].id, 3);
        assert_eq!(drained[1].id, 4);
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn ordered_list_ref() {
        let events = [
            TestEvent { priority: 1, id: 1 },
            TestEvent { priority: 2, id: 2 },
            TestEvent { priority: 3, id: 3 },
        ];

        let mut list = OrderedListRef::<TestEvent>::new(10);
        for event in &events {
            list.insert(event).unwrap();
        }

        assert_eq!(list.len(), 3);
        assert_eq!(list.peek_lowest().unwrap().id, 1);
        assert_eq!(list.peek_highest().unwrap().id, 3);
    }

    #[test]
    fn ordered_list_idx() {
        let mut list = OrderedListIdx::<TestEvent>::new(10);
        list.insert(TestEvent { priority: 1, id: 1 }).unwrap();
        list.insert(TestEvent { priority: 2, id: 2 }).unwrap();
        list.insert(TestEvent { priority: 3, id: 3 }).unwrap();

        assert_eq!(list.len(), 3);
        assert_eq!(list.pop_highest().unwrap().id, 3);
    }
}
