# 1) Use the garage CLI inside the container
alias garage='docker exec -ti garage /garage'

# 2) Assign a layout (single node) and apply it
NODE_ID="$(garage status | awk '/NO ROLE ASSIGNED/{print $1; exit}')"
garage layout assign -z dc1 -c 1G "$NODE_ID"
garage layout apply --version 1

# 3) Create a bucket + key and grant least-privilege-ish permissions
garage bucket create files
garage key create file-storage-key
garage bucket allow --read --write --owner files --key file-storage-key

# 4) Show the key material (save it to .env)
garage key info file-storage-key
