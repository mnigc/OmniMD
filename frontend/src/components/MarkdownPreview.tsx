import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import rehypeRaw from "rehype-raw";
import rehypeSlug from "rehype-slug";
import { useI18n } from "../i18n";

interface MarkdownPreviewProps {
  content: string;
}

export function MarkdownPreview({ content }: MarkdownPreviewProps) {
  const { t } = useI18n();

  if (!content.trim()) {
    return (
      <div className="flex flex-col items-center justify-center h-full text-muted-foreground">
        <div className="w-16 h-16 rounded-full bg-muted flex items-center justify-center mb-3">
          <span className="text-2xl font-bold text-muted-foreground">{`{ }`}</span>
        </div>
        <p className="text-sm">{t("markdown.noContent")}</p>
      </div>
    );
  }

  return (
    <div className="markdown-body">
        <ReactMarkdown
          remarkPlugins={[remarkGfm]}
          rehypePlugins={[rehypeRaw, rehypeSlug]}
          components={{
          code({ className, children, ...props }) {
            const match = /language-(\w+)/.exec(className || "");
            return match ? (
              <pre className="bg-muted border border-border rounded-md p-3 overflow-x-auto my-2">
                <code
                  className={`font-mono text-sm ${className}`}
                  {...props}
                >
                  {children}
                </code>
              </pre>
            ) : (
              <code
                className="bg-muted px-1.5 py-0.5 rounded text-sm font-mono"
                {...props}
              >
                {children}
              </code>
            );
          },
          pre({ children }) {
            return <div className="my-2">{children}</div>;
          },
          img({ src, alt }) {
            return (
              <img
                src={src}
                alt={alt || ""}
                className="max-w-full h-auto rounded-md border border-border my-2"
              />
            );
          },
          table({ children }) {
            return (
              <div className="overflow-x-auto my-3 border border-border rounded-md">
                <table className="min-w-full">{children}</table>
              </div>
            );
          },
          th({ children }) {
            return (
              <th className="px-3 py-2 bg-muted text-left text-sm font-medium border-b border-border">
                {children}
              </th>
            );
          },
          td({ children }) {
            return (
              <td className="px-3 py-2 text-sm border-b border-border last:border-b-0">
                {children}
              </td>
            );
          },
          a({ href, children }) {
            return (
              <a
                href={href}
                className="text-primary hover:underline"
                target="_blank"
                rel="noopener noreferrer"
              >
                {children}
              </a>
            );
          },
          ul({ children }) {
            return <ul className="list-disc list-inside space-y-1 my-2">{children}</ul>;
          },
          ol({ children }) {
            return <ol className="list-decimal list-inside space-y-1 my-2">{children}</ol>;
          },
          blockquote({ children }) {
            return (
              <blockquote className="border-l-4 border-primary/40 pl-4 my-2 text-muted-foreground italic">
                {children}
              </blockquote>
            );
          },
          hr() {
            return <hr className="border-border my-4" />;
          },
        }}
      >
        {content}
      </ReactMarkdown>
    </div>
  );
}