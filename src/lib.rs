use specs::{DenseVecStorage, BitSet, Component, Storage, Join};
use std::collections::{HashMap, HashSet};
use specs::storage::{UnprotectedStorage, TryDefault, MaskedStorage};
use specs::hibitset::{BitSetLike, DrainableBitSet};
use std::hash::Hash;
use std::collections::hash_map::RandomState;
use std::ops::{Deref, DerefMut};

pub trait Grouped<C> {
    fn get_groups(&mut self) -> &HashMap<C, BitSet>;
}

pub trait UnprotectedGrouped<C> {
    unsafe fn get_groups(&self) -> &HashMap<C, BitSet>;
}

impl<'e, T, D> Grouped<T> for Storage<'e, T, D> where
    T: Component,
    T::Storage: UnprotectedGrouped<T>,
    D: DerefMut<Target = MaskedStorage<T>>{

    fn get_groups(&mut self) -> &HashMap<T, BitSet> {
        unsafe {
            let (mask, storage) = self.open();
            storage.get_and_maintain_groups(mask)
        }
    }
}

pub struct GroupedStorage<C, T = DenseVecStorage<C>> {
    groups: HashMap<C, BitSet>,
    dirty_entitys: BitSet,
    dirty_groups: HashSet<C>,
    empty_groups: Vec<C>,
    storage: T
}

impl<C, T: UnprotectedStorage<C>> GroupedStorage<C, T> {
    fn add_to_group(&mut self, id: u32, component: C) {
        self.groups.entry(component).or_default()
            .add(id);
    }

    fn remove_from_group(&mut self, id: u32, component: &C) {
        if let Some(group) = self.groups.get_mut(component) {
            group.remove(id);
            if group.is_empty() {
                self.dirty_groups.insert(component.clone());
            }
        }
    }
}

impl<C: Hash + Eq + Clone, T: UnprotectedStorage<C>> UnprotectedGrouped<C> for GroupedStorage<C, T> {
    unsafe fn get_groups(&mut self) -> &HashMap<C, BitSet> {
        for id in &self.dirty_entitys.drain() {
            let component = self.storage.get(id);
            self.add_to_group(id, component.clone())
        }

        for dirty_group in self.dirty_groups.drain() {
            if let Some(group) = self.groups.get(&dirty_group) {
                if group.is_empty() {
                    self.empty_groups.push(dirty_group)
                }
            }
        }

        for empty_group in self.empty_groups.drain(..) {
            self.groups.remove(&empty_group);
        }

        &self.groups
    }
}

impl<C: Hash + Eq, T: UnprotectedStorage<C>> TryDefault for GroupedStorage<C, T> {
    fn try_default() -> Result<Self, String> {
        Ok(Self {
            groups: HashMap::new(),
            dirty_entitys: BitSet::new(),
            dirty_groups: HashSet::new(),
            empty_groups: Vec::new(),
            storage: T::try_default()?
        })
    }
}

impl<C: Component + Hash + Eq + Clone, T: UnprotectedStorage<C>> UnprotectedStorage<C> for GroupedStorage<C, T> {
    unsafe fn clean<B>(&mut self, has: B) where B: BitSetLike {
        self.storage.clean(has)
    }

    unsafe fn get(&self, id: u32) -> &C {
        self.storage.get(id)
    }

    unsafe fn get_mut(&mut self, id: u32) -> &mut C {
        let component = self.storage.get_mut(id);

        self.dirty_entitys.add(id);
        self.remove_from_group(id, component);

        component
    }

    unsafe fn insert(&mut self, id: u32, value: C) {
        self.dirty_entitys.add(id);
        self.storage.insert(id, value)
    }

    unsafe fn remove(&mut self, id: u32) -> C {
        let component = self.storage.remove(id);

        self.dirty_entitys.remove(id);
        self.remove_from_group(id, &component);

        component
    }
}
