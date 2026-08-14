/// <reference types="vite/client" />

interface FileSystemEntry {
  isFile: boolean;
  isDirectory: boolean;
  name: string;
}

interface FileSystemFileEntry extends FileSystemEntry {
  file(successCallback: (file: File) => void): void;
}

interface FileSystemDirectoryEntry extends FileSystemEntry {
  createReader(): FileSystemDirectoryReader;
}

interface FileSystemDirectoryReader {
  readEntries(successCallback: (entries: FileSystemEntry[]) => void): void;
}
