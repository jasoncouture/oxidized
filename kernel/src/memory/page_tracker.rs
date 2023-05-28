use bitvec::vec::BitVec;

#[derive(Debug)]
pub struct PageTracker {
    data: BitVec<u8>
}

pub enum PageTrackerError {
    NotFound
}

impl PageTracker {
    pub fn new(length: usize) -> PageTracker {
        let mut ret = PageTracker { data: BitVec::new() };
        ret.data.resize(length, false);
        ret
    }

    pub fn is_used(&self, index: usize) -> bool {
        self.data.len() > index && self.data[index]
    }

    pub fn set_state(&mut self, index: usize, state: bool) {
        
        if self.data.len() <= index {
            self.data.resize(index, false);
        }
        self.data.set(index, state);
    }

    pub fn reserve(&mut self, index: usize) {
        self.set_state(index, true)
    }

    pub fn reserve_range(&mut self, index: usize, count: usize) {
        for i in index..(index + count) {
            self.reserve(i)
        }
    }

    pub fn free_range(&mut self, index: usize, count: usize) {
        for i in index..(index + count) {
            self.free(i)
        }
    }


    pub fn free(&mut self, index: usize) {
        self.set_state(index, false)
    }

    pub fn find_free_range(&self, start_index: usize, count: usize) -> Result<usize, PageTrackerError> {
        todo!()
    }

    pub fn find_free(&self, start_index: usize) -> Result<usize, PageTrackerError> {
        let first_zero = match self.data.first_zero() {
            Some(idx) => idx,
            None => return Err(PageTrackerError::NotFound)
        };
        let mut start_index = 
        if first_zero > start_index {
            first_zero
        } else {
            start_index
        };

        while start_index < self.data.len() {
            if self.data[start_index] == false {
                return Ok(start_index)
            } else {
                start_index = start_index+1;
            }
        }

        Err(PageTrackerError::NotFound)
    }
}