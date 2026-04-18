import keycloak from "../auth/keycloak";
import type { FileMetadata } from "./schemas";
import {
  FileMetadataArraySchema,
  CheckFileExistsSchema,
  PresignedUrlSchema,
} from "./schemas";

class FileStorageClient {
  readonly baseUrl: string;

  constructor() {
    this.baseUrl = "/api";
  }

  private async getToken() {
    let token = "";
    if (keycloak.token) {
      // Refresh token if needed
      await keycloak.updateToken(30); // Refresh if expires in 30s
      token = keycloak.token;
    }
    return token;
  }

  async getAllFiles(): Promise<FileMetadata[]> {
    const token = await this.getToken();
    const response = await fetch(`${this.baseUrl}/files`, {
      method: "GET",
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });

    if (!response.ok) {
      const details = await response.text();
      throw new Error(
        `Response returned with status ${response.status}: ${details}`,
      );
    }

    const jsonData = await response.json();
    const validated = await FileMetadataArraySchema.parseAsync(jsonData);
    const data: FileMetadata[] = [];
    for (const val of validated.files) {
      data.push(val as FileMetadata);
    }

    return data;
  }

  async uploadFile(file: File, fileDescription: string) {
    const formData = new FormData();
    formData.append("file", file);
    formData.append("description", fileDescription);
    const displayName = file.name;
    const token = await this.getToken();
    const exists = await fetch(`${this.baseUrl}/checks/${displayName}`, {
      method: "GET",
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    if (!exists.ok) {
      const details = await exists.text();
      throw new Error(
        `Response returned with status ${exists.status}: ${details}`,
      );
    }
    const existsData = await exists.json();
    const existsVal = await CheckFileExistsSchema.parseAsync(existsData);
    const newDisplayName = existsVal.file_name;
    formData.append("file_name", newDisplayName);
    const response = await fetch(`${this.baseUrl}/uploads`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${token}`,
      },
      body: formData,
    });
    if (!response.ok) {
      const details = await response.text();
      throw new Error(
        `Response returned with status ${response.status}: ${details}`,
      );
    }
    return newDisplayName;
  }

  async getPresignedUrl(fileName: string) {
    const token = await this.getToken();
    const response = await fetch(`${this.baseUrl}/urls/${fileName}`, {
      method: "GET",
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    if (!response.ok) {
      const details = await response.text();
      throw new Error(
        `Response returned with status ${response.status}: ${details}`,
      );
    }
    const responseData = await response.json();
    const valData = await PresignedUrlSchema.parseAsync(responseData);
    return valData.presigned_url;
  }

  async deleteFile(fileName: string) {
    const token = await this.getToken();
    const response = await fetch(`${this.baseUrl}/files/${fileName}`, {
      method: "DELETE",
      headers: {
        Authorization: `Bearer ${token}`,
      },
    });
    if (!response.ok) {
      const details = await response.text();
      throw new Error(
        `Response returned with status ${response.status}: ${details}`,
      );
    }
  }
}

const fileStorageClient = new FileStorageClient();

export function getFileStorageClient() {
  return fileStorageClient;
}
