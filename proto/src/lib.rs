pub mod provisioner {
    use shuttle_common::DatabaseReadyInfo;

    tonic::include_proto!("provisioner");

    impl From<DatabaseResponse> for DatabaseReadyInfo {
        fn from(response: DatabaseResponse) -> Self {
            DatabaseReadyInfo {
                role_name: response.username,
                role_password: response.password,
                database_name: response.database_name,
            }
        }
    }
}
