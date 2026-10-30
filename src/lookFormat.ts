// Which kind of look a picked file is, from its name alone: the picker lets any
// file through on Android, so this refuses the others before reading them.

export function lookFormat(fileName: string): "cube" | "png" {
  if (/\.cube$/i.test(fileName)) return "cube";
  if (/\.png$/i.test(fileName)) return "png";
  throw new Error("Only .cube and .png files can be added as looks.");
}
