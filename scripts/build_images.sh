cd backend
docker build . -f rest.Dockerfile -t file-storage-rest
docker build . -f grpc.Dockerfile -t file-storage-grpc
docker build . -f qdrant.Dockerfile -t file-storage-qdrant-server
docker build . -f queue.Dockerfile -t file-storage-queue-worker
cd ../frontend
docker build . -t file-storage-frontend
cd ..
