use crate::entity::EntityId;

// Represents a relationship between two entities (Relationship, Target).
//
// A relationship can only consists of valid and alive entities,
// except for special cases where appropriate.
//
// Pairs can contain data if either relationship or target has associated data.
// To get the data type, the relationship is first considered, then the target.
struct Pair(EntityId, EntityId);
