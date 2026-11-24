/// A JPEG's metadata only, with one EXIF orientation entry. There are no
/// compressed pixels: browser smoke tests generate a real JPEG separately.
export function exifJpeg(orientation: number, littleEndian = false): Uint8Array<ArrayBuffer> {
    const bytes = new Uint8Array(40)
    bytes.set([0xff, 0xd8, 0xff, 0xe1, 0, 34, 69, 120, 105, 102, 0, 0])
    const tiff = new DataView(bytes.buffer, 12, 26)
    tiff.setUint16(0, littleEndian ? 0x4949 : 0x4d4d)
    tiff.setUint16(2, 42, littleEndian)
    tiff.setUint32(4, 8, littleEndian)
    tiff.setUint16(8, 1, littleEndian)
    tiff.setUint16(10, 0x0112, littleEndian)
    tiff.setUint16(12, 3, littleEndian)
    tiff.setUint32(14, 1, littleEndian)
    tiff.setUint16(18, orientation, littleEndian)
    bytes.set([0xff, 0xd9], 38)
    return bytes
}
