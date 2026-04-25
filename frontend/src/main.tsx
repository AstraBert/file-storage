import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import keycloak from "./auth/keycloak";
import NoAuth from "./NoAuth";
import { BrowserRouter, Routes, Route } from "react-router-dom";
import Search from "./pages/Search";

keycloak
  .init({ onLoad: "login-required", checkLoginIframe: false })
  .then((authenticated) => {
    if (authenticated) {
      ReactDOM.createRoot(document.getElementById("root")!).render(
        <React.StrictMode>
          <BrowserRouter>
            <Routes>
              <Route path="/" element={<App />} />
              <Route path="/search" element={<Search />} />
            </Routes>
          </BrowserRouter>
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
