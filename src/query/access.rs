use crate::component::ComponentId;

/// Error returned when a query's terms would alias.
#[derive(Debug, thiserror::Error)]
pub enum AccessError {
    #[error("component {0} is written by more than one term")]
    DoubleWrite(ComponentId),
    #[error("component {0} is both read and written")]
    ReadWrite(ComponentId),
}

/// The components a query reads and writes, collected from its terms.
#[derive(Default)]
pub struct AccessSet {
    reads: Vec<ComponentId>,
    writes: Vec<ComponentId>,
}

impl AccessSet {
    /// Records a read of `c`. Any number of terms may read a component that
    /// no term writes.
    pub fn read(&mut self, c: ComponentId) -> Result<(), AccessError> {
        if self.writes.contains(&c) {
            return Err(AccessError::ReadWrite(c));
        }
        if !self.reads.contains(&c) {
            self.reads.push(c);
        }
        Ok(())
    }

    /// Records a write of `c`. A written component is accessed by no other term.
    pub fn write(&mut self, c: ComponentId) -> Result<(), AccessError> {
        if self.writes.contains(&c) {
            return Err(AccessError::DoubleWrite(c));
        }
        if self.reads.contains(&c) {
            return Err(AccessError::ReadWrite(c));
        }
        self.writes.push(c);
        Ok(())
    }
}
