use std::rc::Rc;

use super::ComponentId;

pub struct TraitInfo {
    pub name: Rc<str>,
    components: Vec<ComponentId>,
}
