//! 已收字节区间集合（实施方案 4.2/7.2）。
//!
//! 接收端持久化各文件已收区间；支持乱序到达与区间合并。
//! 区间均为左闭右开 `[start, end)`。

use serde::{Deserialize, Serialize};

/// 有序、已合并的区间集合。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeSet {
    intervals: Vec<(u64, u64)>,
}

impl RangeSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_intervals(mut intervals: Vec<(u64, u64)>) -> Self {
        intervals.sort_unstable();
        let mut set = RangeSet {
            intervals: Vec::new(),
        };
        for (s, e) in intervals {
            set.insert(s, e);
        }
        set
    }

    /// 插入区间 `[start, end)` 并合并相邻/重叠区间。
    pub fn insert(&mut self, start: u64, end: u64) {
        if end <= start {
            return;
        }
        let mut new_intervals = Vec::with_capacity(self.intervals.len() + 1);
        let mut merged = (start, end);
        let mut inserted = false;

        for &(s, e) in &self.intervals {
            if e < merged.0 {
                new_intervals.push((s, e));
            } else if s > merged.1 {
                if !inserted {
                    new_intervals.push(merged);
                    inserted = true;
                }
                new_intervals.push((s, e));
            } else {
                merged.0 = merged.0.min(s);
                merged.1 = merged.1.max(e);
            }
        }
        if !inserted {
            new_intervals.push(merged);
        }
        self.intervals = new_intervals;
    }

    /// 区间是否完全覆盖 `[0, size)`。
    pub fn covers_all(&self, size: u64) -> bool {
        if size == 0 {
            return true;
        }
        matches!(self.intervals.as_slice(), [(0, e)] if *e >= size)
    }

    /// 前缀连续覆盖到的偏移（累积确认值）。
    pub fn contiguous_prefix(&self) -> u64 {
        self.intervals
            .first()
            .filter(|(s, _)| *s == 0)
            .map(|(_, e)| *e)
            .unwrap_or(0)
    }

    /// 是否已覆盖 `[offset, offset+len)`。
    pub fn covers(&self, offset: u64, len: u64) -> bool {
        if len == 0 {
            return true;
        }
        let end = offset.saturating_add(len);
        for &(s, e) in &self.intervals {
            if s > offset {
                break;
            }
            if s <= offset && e >= end {
                return true;
            }
        }
        false
    }

    /// 已收字节总数。
    pub fn total_received(&self) -> u64 {
        self.intervals.iter().map(|(s, e)| e - s).sum()
    }

    /// 未覆盖的待传区间（发送端跳过已收分片用）。
    pub fn gaps(&self, size: u64) -> Vec<(u64, u64)> {
        let mut gaps = Vec::new();
        let mut cursor = 0u64;
        for &(s, e) in &self.intervals {
            if s > cursor {
                gaps.push((cursor, s.min(size)));
            }
            cursor = cursor.max(e);
            if cursor >= size {
                return gaps;
            }
        }
        if cursor < size {
            gaps.push((cursor, size));
        }
        gaps
    }

    pub fn intervals(&self) -> &[(u64, u64)] {
        &self.intervals
    }

    pub fn is_empty(&self) -> bool {
        self.intervals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_adjacent_and_overlapping() {
        let mut r = RangeSet::new();
        r.insert(0, 10);
        r.insert(10, 20); // adjacent merge
        assert_eq!(r.intervals(), &[(0, 20)]);
        r.insert(30, 40);
        r.insert(15, 35); // bridge
        assert_eq!(r.intervals(), &[(0, 40)]);
        assert_eq!(r.total_received(), 40);
    }

    #[test]
    fn out_of_order_and_duplicates() {
        let mut r = RangeSet::new();
        r.insert(100, 200);
        r.insert(0, 50);
        r.insert(0, 50); // duplicate
        r.insert(40, 110);
        assert_eq!(r.intervals(), &[(0, 200)]);
        assert!(r.covers_all(200));
        assert!(!r.covers_all(201));
    }

    #[test]
    fn prefix_and_gaps() {
        let mut r = RangeSet::new();
        r.insert(0, 10);
        r.insert(20, 30);
        assert_eq!(r.contiguous_prefix(), 10);
        assert_eq!(r.gaps(50), vec![(10, 20), (30, 50)]);
        assert!(r.covers(5, 5));
        assert!(!r.covers(5, 10));
    }

    #[test]
    fn empty_file_covered() {
        let r = RangeSet::new();
        assert!(r.covers_all(0));
        assert_eq!(r.gaps(0), vec![]);
    }
}
