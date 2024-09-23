fn main() {
    tonic_build::compile_protos("proto/my_service.proto").unwrap();
}