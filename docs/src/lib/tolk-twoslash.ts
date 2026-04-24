import type {
  NodeError,
  TwoslashGenericFunction,
  TwoslashNode,
} from 'twoslash-protocol';

type PendingNode = Omit<NodeError, 'start'> & { start?: number };

const markerPattern = /^(\s*\/\/\s*)(\^+)\s*(.*)$/;

export const tolkTwoslasher: TwoslashGenericFunction = (code) => {
  const inputLines = code.replace(/\r\n/g, '\n').split('\n');
  const outputLines: string[] = [];
  const pendingNodes: PendingNode[] = [];
  let previousOutputLine = -1;

  for (const line of inputLines) {
    const marker = line.match(markerPattern);

    if (marker && previousOutputLine >= 0) {
      const character = marker[1].length;
      const length = marker[2].length;
      const annotation = marker[3].trim();
      const targetLine = outputLines[previousOutputLine] ?? '';
      const safeCharacter = Math.min(character, Math.max(targetLine.length - 1, 0));
      const safeLength = Math.max(1, Math.min(length, Math.max(targetLine.length - safeCharacter, 1)));

      if (annotation.length > 0) {
        pendingNodes.push({
          type: 'error',
          line: previousOutputLine,
          character: safeCharacter,
          length: safeLength,
          text: annotation,
          level: 'warning',
          id: `tolk-${previousOutputLine}-${safeCharacter}`,
        });
      }

      continue;
    }

    outputLines.push(line);
    previousOutputLine = outputLines.length - 1;
  }

  const outputCode = outputLines.join('\n');
  const lineStarts = getLineStarts(outputLines);
  const nodes = pendingNodes.map((node) => ({
    ...node,
    start: lineStarts[node.line] + node.character,
  })) as TwoslashNode[];

  return {
    code: outputCode,
    extension: 'tolk',
    nodes,
  };
};

function getLineStarts(lines: string[]): number[] {
  const starts: number[] = [];
  let offset = 0;

  for (const line of lines) {
    starts.push(offset);
    offset += line.length + 1;
  }

  return starts;
}
