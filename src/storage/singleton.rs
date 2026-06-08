use std::rc::Rc;

use crate::{
    Id,
    storage::{
        BorrowMutError, BorrowRefError,
        borrow::{BorrowMut, BorrowRef},
        column::Column,
    },
    type_meta::TypeMeta,
};

pub struct Singleton {
    data: Column,
}

impl Drop for Singleton {
    fn drop(&mut self) {
        unsafe { self.data.destroy(1, 1) }
    }
}

impl Singleton {
    pub(crate) fn new<T: 'static>(id: Id, type_meta: Rc<TypeMeta>, val: T) -> Self {
        let mut data = Column::new(id, type_meta);

        unsafe {
            data.realloc(0, 1);
            data.write(1, val);
        }

        Self { data }
    }

    fn get<T: 'static>(&self) -> &T {
        self.data.assert_type::<T>();
        unsafe { &(*self.data.as_ptr()) }
    }

    fn get_mut<T: 'static>(&self) -> &mut T {
        self.data.assert_type::<T>();
        unsafe { &mut (*self.data.as_ptr()) }
    }

    #[inline(always)]
    pub(crate) fn borrow<T: 'static>(&self) -> SingletonRef<'_, T> {
        SingletonRef {
            borrow: self.data.borrow.borrow(),
            value: self.get(),
        }
    }

    #[inline(always)]
    pub(crate) fn borrow_mut<T: 'static>(&self) -> SingletonMut<'_, T> {
        SingletonMut {
            borrow: self.data.borrow.borrow_mut(),
            value: self.get_mut(),
        }
    }

    #[inline(always)]
    pub(crate) fn try_borrow<T: 'static>(&self) -> Result<SingletonRef<'_, T>, BorrowRefError> {
        let borrow = self.data.borrow.try_borrow();
        borrow.map(|b| SingletonRef { borrow: b, value: self.get() })
    }

    #[inline(always)]
    pub(crate) fn try_borrow_mut<T: 'static>(&self) -> Result<SingletonMut<'_, T>, BorrowMutError> {
        let borrow = self.data.borrow.try_borrow_mut();
        borrow.map(|b| SingletonMut { borrow: b, value: self.get_mut() })
    }
}

pub struct SingletonRef<'a, T> {
    value: &'a T,
    #[allow(dead_code)] // held to release the borrow on drop
    borrow: BorrowRef<'a>,
}

pub struct SingletonMut<'a, T> {
    value: &'a mut T,
    #[allow(dead_code)] // held to release the borrow on drop
    borrow: BorrowMut<'a>,
}

impl<T> std::ops::Deref for SingletonRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T> std::ops::Deref for SingletonMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T> std::ops::DerefMut for SingletonMut<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}
