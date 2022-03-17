pub mod discovery;
pub mod gd_udp;
pub mod gossip;
pub mod node;
pub mod protocol;
pub mod record;
pub mod traits;
pub mod transport;
pub mod utils;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
