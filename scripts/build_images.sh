cd backend
docker build . -f rest.Dockerfile -t file-storage-rest
docker build . -f grpc.Dockerfile -t file-storage-grpc
cd ../frontend
docker build . -t file-storage-frontend
cd ..
