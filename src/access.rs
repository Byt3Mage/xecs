use std::{ops::Deref, rc::Rc};

use crate::{Id, validate::WriteAccessError};

/// How a field accesses its component column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AccessType {
    Read,
    Write,
}

impl AccessType {
    pub const fn is_read(&self) -> bool {
        matches!(self, AccessType::Read)
    }

    pub const fn is_write(&self) -> bool {
        matches!(self, AccessType::Write)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access {
    pub id: Id,
    pub ty: AccessType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StaticAccess {
    pub id: u32,
    pub ty: AccessType,
}

#[derive(Debug, Default, Clone)]
pub struct AccessList {
    list: Vec<Access>,
}

impl AccessList {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self { list: Vec::with_capacity(capacity) }
    }

    pub fn push(&mut self, new: Access) -> Result<(), WriteAccessError> {
        crate::validate::check_push(&self.list, new)?;
        self.list.push(new);
        Ok(())
    }
}

impl Deref for AccessList {
    type Target = [Access];

    fn deref(&self) -> &Self::Target {
        &self.list
    }
}

impl From<AccessList> for Rc<[Access]> {
    fn from(value: AccessList) -> Self {
        Rc::from(value.list)
    }
}
