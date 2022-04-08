pub mod config;
pub mod console;
pub mod graph;
pub mod paths;
pub mod util;
pub mod viewer;

pub mod graph_3d_app;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
