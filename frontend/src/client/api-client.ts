import keycloak from "../auth/keycloak";
import type { FileMetadata } from "./schemas";
import { FileMetadataArraySchema } from "./schemas";

export class FileStorageClient {
  readonly baseUrl: string;

  constructor() {
    this.baseUrl = "http://rest-server:4444";
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
    const token = this.getToken();
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
    const token = this.getToken();
    const response = await fetch(`files/${displayName}`, {
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
    return file.name;
  }
}
