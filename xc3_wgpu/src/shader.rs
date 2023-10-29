// Include the bindings generated by build.rs.
// Not modifying the src directory makes this crate easier to publish.
pub mod deferred {
    include!(concat!(env!("OUT_DIR"), "/deferred.rs"));
}
pub mod model {
    include!(concat!(env!("OUT_DIR"), "/model.rs"));
}
