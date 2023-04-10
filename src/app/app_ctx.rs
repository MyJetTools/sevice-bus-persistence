use std::{sync::Arc, time::Duration};

use my_azure_storage_sdk::{page_blob::AzurePageBlobStorage, AzureStorageConnection};
use my_service_bus_shared::page_id::PageId;
use rust_extensions::AppStates;

use crate::{
    index_by_minute::{IndexByMinuteStorage, IndexByMinuteUtils},
    page_blob_random_access::PageBlobRandomAccess,
    settings::SettingsModel,
    toipics_snapshot::current_snapshot::CurrentTopicsSnapshot,
    topic_data::TopicsDataList,
};

use super::{logs::Logs, PrometheusMetrics};

pub const APP_VERSION: &'static str = env!("CARGO_PKG_VERSION");

pub const PAGE_BLOB_MAX_PAGES_TO_UPLOAD_PER_ROUND_TRIP: usize = 1024 * 1024 * 3 / 512;

pub struct AppContext {
    pub app_states: Arc<AppStates>,
    pub topics_snapshot: CurrentTopicsSnapshot,
    pub logs: Arc<Logs>,

    pub topics_list: TopicsDataList,
    pub settings: SettingsModel,
    pub queue_connection: AzureStorageConnection,
    pub messages_connection: Arc<AzureStorageConnection>,

    pub metrics_keeper: PrometheusMetrics,
    pub index_by_minute_utils: IndexByMinuteUtils,

    messages_conn_string: Arc<AzureStorageConnection>,
    pub grpc_timeout: Duration,
}

impl AppContext {
    pub async fn new(settings: SettingsModel) -> AppContext {
        let logs = Arc::new(Logs::new());
        let messages_connection = Arc::new(AzureStorageConnection::from_conn_string(
            settings.messages_connection_string.as_str(),
        ));

        let queue_connection =
            AzureStorageConnection::from_conn_string(settings.queues_connection_string.as_str());

        let topics_repo = settings.get_topics_snapshot_repository().await;

        let messages_conn_string =
            AzureStorageConnection::from_conn_string(settings.messages_connection_string.as_str());

        AppContext {
            topics_snapshot: CurrentTopicsSnapshot::new(topics_repo).await,
            logs: logs.clone(),
            topics_list: TopicsDataList::new(),
            settings,

            messages_connection,
            queue_connection,
            metrics_keeper: PrometheusMetrics::new(),
            index_by_minute_utils: IndexByMinuteUtils::new(),
            messages_conn_string: Arc::new(messages_conn_string),
            app_states: Arc::new(AppStates::create_un_initialized()),
            grpc_timeout: Duration::from_secs(5),
        }
    }

    pub fn get_max_payload_size(&self) -> usize {
        1024 * 1024 * 3 //TODO - сделать настройку
    }

    pub fn get_max_message_size(&self) -> usize {
        1024 * 1024 * 5 //TODO - сделать настройку
    }

    pub fn get_env_info(&self) -> String {
        let env_info = std::env::var("ENV_INFO");

        match env_info {
            Ok(info) => info,
            Err(err) => format!("{:?}", err),
        }
    }

    pub async fn open_uncompressed_page_storage_if_exists(
        &self,
        topic_id: &str,
        page_id: PageId,
    ) -> Option<PageBlobRandomAccess> {
        let blob_name = super::file_name_generators::generate_uncompressed_blob_name(page_id);

        let azure_storage = AzurePageBlobStorage::new(
            self.messages_conn_string.clone(),
            topic_id.to_string(),
            blob_name,
        )
        .await;

        PageBlobRandomAccess::open_if_exists(
            azure_storage,
            PAGE_BLOB_MAX_PAGES_TO_UPLOAD_PER_ROUND_TRIP,
        )
        .await
    }

    pub async fn open_or_create_uncompressed_page_storage(
        &self,
        topic_id: &str,
        page_id: PageId,
    ) -> PageBlobRandomAccess {
        let blob_name = super::file_name_generators::generate_uncompressed_blob_name(page_id);

        let azure_storage = AzurePageBlobStorage::new(
            self.messages_conn_string.clone(),
            topic_id.to_string(),
            blob_name,
        )
        .await;

        PageBlobRandomAccess::open_or_create(
            azure_storage,
            PAGE_BLOB_MAX_PAGES_TO_UPLOAD_PER_ROUND_TRIP,
        )
        .await
    }

    pub async fn create_topic_folder(&self, topic_folder: &str) {
        super::azure_storage_with_retries::create_container_if_not_exists(
            self.messages_conn_string.as_ref(),
            topic_folder,
        )
        .await;
    }

    pub async fn create_index_storage(&self, topic_id: &str, year: u32) -> IndexByMinuteStorage {
        let blob_name = super::file_name_generators::generate_year_index_blob_name(year);

        let azure_storage = AzurePageBlobStorage::new(
            self.messages_conn_string.clone(),
            topic_id.to_string(),
            blob_name,
        )
        .await;

        let page_blob_random_access = PageBlobRandomAccess::open_or_create(
            azure_storage,
            PAGE_BLOB_MAX_PAGES_TO_UPLOAD_PER_ROUND_TRIP,
        )
        .await;

        IndexByMinuteStorage::new(page_blob_random_access)
    }
}
