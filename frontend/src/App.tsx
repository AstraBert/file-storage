import { useEffect, useRef, useState } from "react";
import "./App.css";
import { getFileStorageClient } from "./client/api-client";
import type { FileMetadata } from "./client/schemas";
import { LoadingSpinner } from "./components/spinner";
import { FilesTable } from "./components/table";
import { Banner } from "./components/banner";
import { Input } from "./components/ui/input";
import { Button } from "./components/ui/button";
import { Label } from "./components/ui/label";

function App() {
  const client = getFileStorageClient();
  const [files, setFiles] = useState<FileMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [fileDescription, setFileDescription] = useState<string>("");
  const [uploading, setUploading] = useState<boolean>(false);
  const [uploadingError, setUploadingError] = useState<string | null>(null);
  const [uploadingSuccess, setUploadingSuccess] = useState<string | null>(null);
  useEffect(() => {
    const fetchFiles = async () => {
      try {
        const data = await client.getAllFiles();
        setFiles(data);
      } catch (err) {
        setError(`An error occurred while loading files: ${err}`);
      } finally {
        setLoading(false);
      }
    };

    fetchFiles();
  }, [client]);

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = event.target.files?.[0];
    if (selectedFile) setFile(selectedFile);
  };

  const handleClear = () => {
    setFile(null);
    setFileDescription("");
    if (fileInputRef.current) fileInputRef.current.value = "";
  };

  const handleDescriptionChange = (
    event: React.ChangeEvent<HTMLInputElement>,
  ) => {
    setFileDescription(event.target.value);
  };

  const handleFileUpload = async () => {
    if (!file) return;
    setUploading(true);
    try {
      await client.uploadFile(file, fileDescription);
      setUploadingSuccess("File successfully uploaded");
      setTimeout(() => {
        setUploadingSuccess(null);
      }, 2000);
    } catch (e) {
      setUploadingError(`An error occurred while uploading the file: ${e}`);
      setTimeout(() => {
        setUploadingError(null);
      }, 10_000);
    } finally {
      setUploading(false);
    }
  };

  return (
    <main>
      <div className="flex flex-col items-center gap-6 flex-1">
        <div className="flex flex-col items-center align-top">
          <h1 className="text-3xl font-bold bg-linear-to-r from-gray-400 to-gray-600 bg-clip-text text-transparent">
            file-storage
          </h1>
          <h2 className="text-xl mb-2">
            A self-hostable, open-source app to store all your files
          </h2>
        </div>
        {loading && <LoadingSpinner message="Loading your files..." />}
        {!loading && files.length > 0 && (
          <div className="flex flex-col items-center space-y-8">
            <FilesTable files={files} />
            <Input type="file">Upload a File</Input>
          </div>
        )}
        {!loading && error && <Banner error={true} message={error} />}
        <div className="flex flex-col mt-10 items-center gap-6 w-full max-w-md mx-auto">
          {/* File input */}
          <div className="w-full space-y-1.5">
            <Label
              htmlFor="file"
              className="text-sm font-medium text-foreground"
            >
              File
            </Label>
            <Input
              type="file"
              id="file"
              name="file"
              onChange={handleFileChange}
              required
              ref={fileInputRef}
              className="cursor-pointer file:mr-3 file:px-3 file:py-1 file:rounded-md file:border-0 file:text-sm file:font-medium file:bg-primary file:text-primary-foreground hover:file:bg-primary/90"
            />
          </div>

          {/* Description input */}
          <div className="w-full space-y-1.5">
            <Label
              htmlFor="description"
              className="text-sm font-medium text-foreground"
            >
              Description
              <span className="ml-1.5 text-xs font-normal text-muted-foreground">
                (optional)
              </span>
            </Label>
            <Input
              type="text"
              onChange={handleDescriptionChange}
              id="description"
              name="description"
              placeholder="Insert file description here"
              className="w-full"
            />
          </div>

          {/* Actions */}
          <div className="flex w-full gap-3">
            <Button
              disabled={!file}
              variant="default"
              className="flex-1 rounded-lg shadow-sm"
              onClick={handleFileUpload}
            >
              Submit
            </Button>
            <Button
              variant="destructive"
              className="flex-1 rounded-lg shadow-sm"
              onClick={handleClear}
            >
              Clear
            </Button>
          </div>

          {/* Feedback */}
          {uploading && <LoadingSpinner message="Uploading your file..." />}
          {!uploading && uploadingError && (
            <Banner error={true} message={uploadingError} />
          )}
          {!uploading && uploadingSuccess && (
            <Banner error={false} message={uploadingSuccess} />
          )}
        </div>
      </div>
    </main>
  );
}

export default App;
