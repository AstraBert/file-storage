import { useRef, useState } from "react";
import "./App.css";
import { getFileStorageClient } from "./client/api-client";
import { LoadingSpinner } from "./components/spinner";
import { FilesTable } from "./components/table";
import { Banner } from "./components/banner";
import { Input } from "./components/ui/input";
import { Button } from "./components/ui/button";
import { Label } from "./components/ui/label";
import { useQuery } from "@tanstack/react-query";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";

const queryClient = new QueryClient();

function AppContent() {
  const client = getFileStorageClient();
  const fileInputRef = useRef<HTMLInputElement>(null);
  const descriptionRef = useRef<HTMLInputElement>(null);
  const [file, setFile] = useState<File | null>(null);
  const [fileDescription, setFileDescription] = useState<string>("");
  const [uploading, setUploading] = useState<boolean>(false);
  const [uploadingError, setUploadingError] = useState<string | null>(null);
  const [uploadingSuccess, setUploadingSuccess] = useState<string | null>(null);
  const {
    data: files,
    isLoading,
    error,
  } = useQuery({
    queryKey: ["files"],
    queryFn: () => client.getAllFiles(),
  });

  const handleFileChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    const selectedFile = event.target.files?.[0];
    if (selectedFile) setFile(selectedFile);
  };

  const handleClear = () => {
    setFile(null);
    setFileDescription("");
    if (fileInputRef.current) fileInputRef.current.value = "";
    if (descriptionRef.current) descriptionRef.current.value = "";
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
      await queryClient.invalidateQueries({ queryKey: ["files"] });
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
          <Button variant="link">
            <a href="/search">
              <span className="text-lg mb-2">Search Files</span>
            </a>
          </Button>
        </div>
        {isLoading && <LoadingSpinner message="Loading your files..." />}
        {!isLoading && files && files.length > 0 && (
          <div className="flex flex-col items-center space-y-8">
            <FilesTable files={files} queryClient={queryClient} />
          </div>
        )}
        {!isLoading && error && (
          <Banner error={true} message={`An error occurred: ${error}`} />
        )}
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
              ref={descriptionRef}
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
              className="flex-1 rounded-lg shadow-sm bg-gray-500 text-neutral-100 hover:bg-gray-700 hover:text-neutral-50 disabled:opacity-50"
              onClick={handleFileUpload}
            >
              Submit
            </Button>
            <Button
              variant="default"
              className="flex-1 rounded-lg shadow-sm bg-red-500 text-white hover:bg-red-700 hover:text-white disabled:opacity-50"
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

function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <AppContent />
    </QueryClientProvider>
  );
}

export default App;
