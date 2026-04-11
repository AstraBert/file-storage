use aws_sdk_s3::{Client, presigning::PresigningConfig};
use std::{sync::Arc, time::Duration};
use tonic::{Request, Response, Status};

use crate::grpc::file_storage::{
    DeleteObjectRequest, DeleteObjectResponse, GetPresignedUrlRequest, GetPresignedUrlResponse,
    StoreFileRequest, StoreFileResponse, file_storage_service_server::FileStorageService,
};

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
    async fn store_file(
        &self,
        request: Request<StoreFileRequest>,
    ) -> Result<Response<StoreFileResponse>, Status> {
        let request_inner = request.into_inner();
        let body = aws_sdk_s3::primitives::ByteStream::from(request_inner.file_data);
        self.client
            .put_object()
            .bucket(&request_inner.bucket_name)
            .key(&request_inner.key)
            .body(body)
            .send()
            .await
            .map_err(|e| Status::new(tonic::Code::Internal, e.to_string()))?;
        Ok(Response::new(StoreFileResponse::default()))
    }

    async fn get_presigned_url(
        &self,
        request: Request<GetPresignedUrlRequest>,
    ) -> Result<Response<GetPresignedUrlResponse>, Status> {
        let request_inner = request.into_inner();
        let expires_in = Duration::from_secs(request_inner.expires_in);
        let presigned_request = self
            .client
            .get_object()
            .bucket(&request_inner.bucket_name)
            .key(&request_inner.key)
            .presigned(
                PresigningConfig::expires_in(expires_in)
                    .map_err(|e| Status::new(tonic::Code::Internal, e.to_string()))?,
            )
            .await
            .map_err(|e| Status::new(tonic::Code::Internal, e.to_string()))?;

        Ok(Response::new(GetPresignedUrlResponse {
            presigned_url: presigned_request.uri().to_owned(),
        }))
    }

    async fn delete_object(
        &self,
        request: Request<DeleteObjectRequest>,
    ) -> Result<Response<DeleteObjectResponse>, Status> {
        let request_inner = request.into_inner();
        let mut delete_object_ids: Vec<aws_sdk_s3::types::ObjectIdentifier> = vec![];
        for obj in vec![request_inner.key] {
            let obj_id = aws_sdk_s3::types::ObjectIdentifier::builder()
                .key(obj)
                .build()
                .map_err(|e| Status::new(tonic::Code::Internal, e.to_string()))?;
            delete_object_ids.push(obj_id);
        }

        self.client
            .delete_objects()
            .bucket(&request_inner.bucket_name)
            .delete(
                aws_sdk_s3::types::Delete::builder()
                    .set_objects(Some(delete_object_ids))
                    .build()
                    .map_err(|e| Status::new(tonic::Code::Internal, e.to_string()))?,
            )
            .send()
            .await
            .map_err(|e| Status::new(tonic::Code::Internal, e.to_string()))?;
        Ok(Response::new(DeleteObjectResponse::default()))
    }
}
