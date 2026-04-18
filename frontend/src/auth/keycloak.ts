import Keycloak from "keycloak-js";

const keycloak = new Keycloak({
  url: "http://keycloak:8080",
  realm: "file-storage",
  clientId: "file-storage-frontend",
});

export default keycloak;
