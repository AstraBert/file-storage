import Keycloak from "keycloak-js";

const keycloak = new Keycloak({
  url: "http://localhost:8080",
  realm: "file-storage",
  clientId: "file-storage-frontend",
});

export default keycloak;
