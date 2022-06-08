pub mod provisioner {
    use shuttle_common::DatabaseReadyInfo;

    tonic::include_proto!("provisioner");

    impl From<DatabaseResponse> for DatabaseReadyInfo {
        fn from(response: DatabaseResponse) -> Self {
            DatabaseReadyInfo::new(
                response.username,
                response.password,
                response.database_name,
                "5432".to_string(),
            )
        }
    }
}
