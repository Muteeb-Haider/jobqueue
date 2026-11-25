pub mod db;
pub mod grpc;
pub mod worker;

pub mod proto {
    tonic::include_proto!("jobqueue");
}
