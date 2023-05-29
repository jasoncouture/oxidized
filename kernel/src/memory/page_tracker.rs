use bitvec::vec::BitVec;

#[derive(Debug)]
pub struct PageTracker {
    data: BitVec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub enum PageTrackerError {
    NotFound,
    TrackerSizeCannotBeShrunk
}

impl PageTracker {
    pub fn new(length: usize) -> PageTracker {
        let mut ret = PageTracker {
            data: BitVec::new(),
        };
        // Since length is 0, and no one else has a reference to the tracker yet, we know this will not fail.
        ret.resize(length).unwrap();
        ret.data.resize(length, false);
        ret
    }

    pub fn resize(&mut self, length: usize) -> Result<(), PageTrackerError> {
        if length < self.data.len() {
            return Err(PageTrackerError::TrackerSizeCannotBeShrunk);
        } else if length == self.data.len() {
            // Ok?
            return Ok(());
        }
        self.data.resize(length, false);
        Ok(())
    }

    pub fn is_used(&self, index: usize) -> bool {
        self.data.len() > index && self.data[index]
    }

    pub fn set_state(&mut self, index: usize, state: bool) -> Result<(), PageTrackerError> {
        if self.data.len() <= index {
            return Err(PageTrackerError::NotFound);
        }
        Ok(self.data.set(index, state))
    }

    pub fn reserve(&mut self, index: usize) -> Result<(), PageTrackerError> {
        self.set_state(index, true)
    }

    pub fn reserve_range(&mut self, index: usize, count: usize) -> Result<(), PageTrackerError> {
        for i in index..(index + count) {
            self.reserve(i)?
        }
        Ok(())
    }

    pub fn free_range(&mut self, index: usize, count: usize) -> Result<(), PageTrackerError> {
        for i in index..(index + count) {
            self.free(i)?
        }
        Ok(())
    }

    pub fn free(&mut self, index: usize) -> Result<(), PageTrackerError> {
        self.set_state(index, false)
    }

    pub fn find_free_range(
        &self,
        start_index: usize,
        count: usize,
    ) -> Result<usize, PageTrackerError> {
        let first_zero = match self.data.first_zero() {
            Some(idx) => idx,
            None => return Err(PageTrackerError::NotFound),
        };
        let mut start_index = if first_zero > start_index {
            first_zero
        } else {
            start_index
        };

        let mut end_index = start_index;

        while start_index < self.data.len() && end_index < self.data.len() && (end_index - start_index) < count {
            if self.data[end_index] || self.data[start_index] {
                // start searching at the page after the current end, if this range doesn't fit, start_index + 1 won't either.
                end_index = end_index + 1;
                start_index = end_index;
            } else {
                end_index = end_index + 1;
            }
        }

        if end_index - start_index < count {
            return Err(PageTrackerError::NotFound)
        }
        Ok(start_index)
        
    }

    pub fn find_free(&self, start_index: usize) -> Result<usize, PageTrackerError> {
        let first_zero = match self.data.first_zero() {
            Some(idx) => idx,
            None => return Err(PageTrackerError::NotFound),
        };
        let mut start_index = if first_zero > start_index {
            first_zero
        } else {
            start_index
        };

        while start_index < self.data.len() {
            if self.data[start_index] == false {
                return Ok(start_index);
            } else {
                start_index = start_index + 1;
            }
        }

        Err(PageTrackerError::NotFound)
    }
}
