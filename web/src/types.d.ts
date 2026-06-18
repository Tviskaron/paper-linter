declare module "js-untar" {
  export interface UntarRecord {
    name: string;
    buffer?: ArrayBuffer;
    blob?: Blob;
  }

  export default function untar(buffer: ArrayBuffer): Promise<UntarRecord[]>;
}

declare module "../pkg/paper_linter.js" {
  export default function init(): Promise<unknown>;
  export class PaperLinter {
    constructor();
    add_file(path: string, bytes: Uint8Array): void;
    check(optionsJson: string): string;
    rules_json(): string;
  }
}

interface FileSystemFileHandle {
  kind: "file";
  name: string;
  getFile(): Promise<File>;
}

interface FileSystemDirectoryHandle {
  kind: "directory";
  name: string;
  entries(): AsyncIterableIterator<[string, FileSystemFileHandle | FileSystemDirectoryHandle]>;
}

interface Window {
  showDirectoryPicker?: (options?: { mode?: "read" | "readwrite" }) => Promise<FileSystemDirectoryHandle>;
}
