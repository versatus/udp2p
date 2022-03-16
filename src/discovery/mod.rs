pub mod kad;
pub mod protocol;
pub mod routing;

const MAX_BUCKET_LEN: usize = 30;
const MAX_BUCKETS: usize = 10;
const REFRESH_INTEVAL: u128 = 900_000_000_000;
const KAD_MESSAGE_LEN: usize = 55000;
const REQ_TIMEOUT: usize = 60_000_000_000;
const MAX_ACTIVE_RPCS: usize = 3;
const DEFAULT_N_PEERS: usize = 8;