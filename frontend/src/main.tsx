import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import keycloak from "./auth/keycloak";
import NoAuth from "./NoAuth";

keycloak
  .init({
    onLoad: "login-required",
    checkLoginIframe: false,
  })
  .then((authenticated) => {
    if (authenticated) {
      ReactDOM.createRoot(document.getElementById("root")!).render(
        <React.StrictMode>
          <App />
        </React.StrictMode>,
      );
    } else {
      ReactDOM.createRoot(document.getElementById("root")!).render(
        <React.StrictMode>
          <NoAuth />
        </React.StrictMode>,
      );
    }
  })
  .catch(console.error);
