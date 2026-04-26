use aws_sdk_s3::{Client, presigning::PresigningConfig};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

use crate::grpc::file_storage::{
    DeleteObjectRequest, DeleteObjectResponse, GetPresignedUrlRequest, GetPresignedUrlResponse,
    StoreFileRequest, StoreFileResponse, file_storage_service_server::FileStorageService,
};
use utils::{STATUS_COMPLETED, STATUS_FAILED, STATUS_STARTED};

pub mod grpc {
    #[path = "file_storage.rs"]
    pub mod file_storage;
}

#[derive(Debug)]
pub struct FileStorageServer {
    pub client: Arc<Client>,
}

#[tonic::async_trait]
impl FileStorageService for FileStorageServer {
    #[tracing::instrument]
    async fn store_file(
        &self,
        request: Request<StoreFileRequest>,
    ) -> Result<Response<StoreFileResponse>, Status> {
        let request_inner = request.into_inner();
        tracing::info!(
            event = "store_file",
            status = STATUS_STARTED,
            file = request_inner.key,
        );
        let body = aws_sdk_s3::primitives::ByteStream::from(request_inner.file_data);
        self.client
            .put_object()
            .bucket(&request_inner.bucket_name)
            .key(&request_inner.key)
            .body(body)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(
                    event = "store_file",
                    status = STATUS_FAILED,
                    file = request_inner.key,
                    error = e.to_string(),
                );
                log::error!("{}", e.to_string());
                Status::new(tonic::Code::Internal, e.to_string())
            })?;
        tracing::info!(
            event = "store_file",
            status = STATUS_COMPLETED,
            file = request_inner.key,
        );
        log::debug!("Successfully stored file with key: {}", &request_inner.key);
        Ok(Response::new(StoreFileResponse::default()))
    }

    #[tracing::instrument]
    async fn get_presigned_url(
        &self,
        request: Request<GetPresignedUrlRequest>,
    ) -> Result<Response<GetPresignedUrlResponse>, Status> {
        let request_inner = request.into_inner();
        tracing::info!(
            event = "get_presigned_url",
            status = STATUS_STARTED,
            file = request_inner.key,
            expires_in = request_inner.expires_in,
        );
        let expires_in = Duration::from_secs(request_inner.expires_in);
        let presigned_request = self
            .client
            .get_object()
            .bucket(&request_inner.bucket_name)
            .key(&request_inner.key)
            .presigned(PresigningConfig::expires_in(expires_in).map_err(|e| {
                log::error!("{}", e.to_string());
                Status::new(tonic::Code::Internal, e.to_string())
            })?)
            .await
            .map_err(|e| {
                log::error!("{}", e.to_string());
                tracing::error!(
                    event = "get_presigned_url",
                    status = STATUS_FAILED,
                    file = request_inner.key,
                    expires_in = request_inner.expires_in,
                    error = e.to_string(),
                );
                Status::new(tonic::Code::Internal, e.to_string())
            })?;

        log::debug!(
            "Successfully produced presigned url with {:?}s expiration for {}",
            request_inner.expires_in,
            &request_inner.key
        );
        tracing::info!(
            event = "get_presigned_url",
            status = STATUS_COMPLETED,
            file = request_inner.key,
            expires_in = request_inner.expires_in,
        );
        Ok(Response::new(GetPresignedUrlResponse {
            presigned_url: presigned_request.uri().to_owned(),
        }))
    }

    #[tracing::instrument]
    async fn delete_object(
        &self,
        request: Request<DeleteObjectRequest>,
    ) -> Result<Response<DeleteObjectResponse>, Status> {
        let request_inner = request.into_inner();
        tracing::info!(
            event = "delete_object",
            status = STATUS_STARTED,
            file = request_inner.key,
        );
        let mut delete_object_ids: Vec<aws_sdk_s3::types::ObjectIdentifier> = vec![];
        for obj in vec![request_inner.key.clone()] {
            let obj_id = aws_sdk_s3::types::ObjectIdentifier::builder()
                .key(obj)
                .build()
                .map_err(|e| {
                    log::error!("{}", e.to_string());
                    tracing::error!(
                        event = "delete_object",
                        status = STATUS_FAILED,
                        file = request_inner.key,
                        error = e.to_string(),
                    );
                    Status::new(tonic::Code::Internal, e.to_string())
                })?;
            delete_object_ids.push(obj_id);
        }

        self.client
            .delete_objects()
            .bucket(&request_inner.bucket_name)
            .delete(
                aws_sdk_s3::types::Delete::builder()
                    .set_objects(Some(delete_object_ids))
                    .build()
                    .map_err(|e| {
                        log::error!("{}", e.to_string());
                        tracing::error!(
                            event = "delete_object",
                            status = STATUS_FAILED,
                            file = request_inner.key,
                            error = e.to_string(),
                        );
                        Status::new(tonic::Code::Internal, e.to_string())
                    })?,
            )
            .send()
            .await
            .map_err(|e| {
                log::error!("{}", e.to_string());
                tracing::error!(
                    event = "delete_object",
                    status = STATUS_FAILED,
                    file = request_inner.key,
                    error = e.to_string(),
                );
                Status::new(tonic::Code::Internal, e.to_string())
            })?;
        tracing::info!(
            event = "delete_object",
            status = STATUS_COMPLETED,
            file = request_inner.key,
        );
        log::debug!("Successfully deleted file with key: {}", &request_inner.key);
        Ok(Response::new(DeleteObjectResponse::default()))
    }
}
