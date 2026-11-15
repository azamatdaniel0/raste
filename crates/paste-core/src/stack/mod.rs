use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::models::ItemEntity;

/// Direction for sequential multi-item pasting (`PasteStack`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackDirection {
    /// First-In, First-Out: pastes oldest queued item first.
    Fifo,
    /// Last-In, First-Out: pastes most recently queued item first.
    Lifo,
}

impl StackDirection {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Fifo => "FIFO (First-In, First-Out)",
            Self::Lifo => "LIFO (Last-In, First-Out)",
        }
    }

    pub fn short_label(&self) -> &'static str {
        match self {
            Self::Fifo => "FIFO",
            Self::Lifo => "LIFO",
        }
    }
}

/// Sequential Paste Stack queue mirroring `Paste.PasteStackWindowController`.
#[derive(Debug, Clone)]
pub struct PasteStack {
    items: VecDeque<ItemEntity>,
    pub direction: StackDirection,
    pub is_active: bool,
}

impl Default for PasteStack {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
            direction: StackDirection::Fifo,
            is_active: false,
        }
    }
}

impl PasteStack {
    pub fn new(direction: StackDirection) -> Self {
        Self {
            items: VecDeque::new(),
            direction,
            is_active: false,
        }
    }

    pub fn push(&mut self, item: ItemEntity) {
        if !self.items.iter().any(|existing| existing.identifier == item.identifier) {
            self.items.push_back(item);
        }
    }

    pub fn pop_next(&mut self) -> Option<ItemEntity> {
        match self.direction {
            StackDirection::Fifo => self.items.pop_front(),
            StackDirection::Lifo => self.items.pop_back(),
        }
    }

    pub fn peek_next(&self) -> Option<&ItemEntity> {
        match self.direction {
            StackDirection::Fifo => self.items.front(),
            StackDirection::Lifo => self.items.back(),
        }
    }

    pub fn toggle_direction(&mut self) -> StackDirection {
        self.direction = match self.direction {
            StackDirection::Fifo => StackDirection::Lifo,
            StackDirection::Lifo => StackDirection::Fifo,
        };
        self.direction
    }

    pub fn remove_at(&mut self, index: usize) -> Option<ItemEntity> {
        self.items.remove(index)
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Returns items in the exact order they will be pasted.
    pub fn ordered_items(&self) -> Vec<&ItemEntity> {
        match self.direction {
            StackDirection::Fifo => self.items.iter().collect(),
            StackDirection::Lifo => self.items.iter().rev().collect(),
        }
    }
}
