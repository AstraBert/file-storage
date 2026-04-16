import { useEffect, useState } from "react";
import "./App.css";
import { getFileStorageClient } from "./client/api-client";
import type { FileMetadata } from "./client/schemas";
import { LoadingSpinner } from "./components/spinner";
import { FilesTable } from "./components/table";
import { ErrorBanner } from "./components/error";
import { Input } from "./components/ui/input";

function App() {
  const client = getFileStorageClient();
  const [files, setFiles] = useState<FileMetadata[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  useEffect(() => {
    const fetchFiles = async () => {
      try {
        const data = await client.getAllFiles();
        setFiles(data);
      } catch (err) {
        setError(err.message);
      } finally {
        setLoading(false);
      }
    };

    fetchFiles();
  }, [client]);

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
        {loading && <LoadingSpinner />}
        {!loading && files.length > 0 && (
          <div className="flex flex-col items-center space-y-8">
            <FilesTable files={files} />
            <Input type="file">Upload a File</Input>
          </div>
        )}
        {!loading && error && <ErrorBanner message={error} />}
      </div>
    </main>
  );
}

export default App;
