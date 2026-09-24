// IA'GS photo preparation. Copyright 2026 IA'GS contributors. Apache-2.0.
// Uses Apple's system Vision model locally. No generative image reconstruction.
import Foundation
import ImageIO
import UniformTypeIdentifiers
import CryptoKit

struct Photo: Codable {
    var id: String
    var originalName: String
    var width: Int
    var height: Int
    var originalWidth: Int
    var originalHeight: Int
    var cameraKey: String
    var focal35: Double?
    var digitalZoom: Double?
    var instances: Int
    var coverage: Double
    var warnings: [String]
    var enabled: Bool
    var reviewed: Bool
    var sha256: String
}
struct Manifest: Codable {
    var version = 1
    var app = "IA'GS"
    var algorithm = "Apple Vision foreground instances / sRGB / aspect-preserving resize v1"
    var maxDimension: Int
    var name: String
    var photos: [Photo]
    var complete: Bool
}
struct Stroke: Codable { var erase: Bool; var radius: Double; var points: [[Double]] }
struct Edit: Codable { var id: String; var strokes: [Stroke]; var select: [Double]?; var enabled: Bool; var reviewed: Bool }
let fm = FileManager.default
let color = CGColorSpace(name: CGColorSpace.sRGB)!
func fail(_ message: String) -> NSError { NSError(domain: "IA'GS", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
func writeJSON<T: Encodable>(_ value: T, _ url: URL) throws {
    let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
    try encoder.encode(value).write(to: url, options: .atomic)
}
func emit(_ value: [String: Any]) { if let d = try? JSONSerialization.data(withJSONObject: value), let s = String(data: d, encoding: .utf8) { print(s); fflush(stdout) } }
func cgImage(_ bytes: [UInt8], _ w: Int, _ h: Int, gray: Bool = false) throws -> CGImage {
    let data = CGDataProvider(data: Data(bytes) as CFData)!
    guard let image = CGImage(width: w, height: h, bitsPerComponent: 8, bitsPerPixel: gray ? 8 : 32, bytesPerRow: w * (gray ? 1 : 4), space: gray ? CGColorSpaceCreateDeviceGray() : color, bitmapInfo: gray ? CGBitmapInfo(rawValue: 0) : CGBitmapInfo(rawValue: CGImageAlphaInfo.last.rawValue), provider: data, decode: nil, shouldInterpolate: false, intent: .defaultIntent) else { throw fail("Cannot encode image") }
    return image
}
func savePNG(_ image: CGImage, _ url: URL) throws {
    let temporary = url.deletingLastPathComponent().appendingPathComponent(".\(UUID().uuidString).png")
    guard let dest = CGImageDestinationCreateWithURL(temporary as CFURL, UTType.png.identifier as CFString, 1, nil) else { throw fail("Cannot write \(url.lastPathComponent)") }
    CGImageDestinationAddImage(dest, image, [kCGImagePropertyOrientation: 1] as CFDictionary)
    guard CGImageDestinationFinalize(dest) else { throw fail("PNG encoding failed") }
    if fm.fileExists(atPath: url.path) { _ = try fm.replaceItemAt(url, withItemAt: temporary) } else { try fm.moveItem(at: temporary, to: url) }
}
func readCG(_ url: URL) throws -> CGImage {
    guard let s = CGImageSourceCreateWithURL(url as CFURL, nil), let image = CGImageSourceCreateImageAtIndex(s, 0, nil) else { throw fail("Cannot decode \(url.lastPathComponent)") }; return image
}
func rgbBytes(_ image: CGImage) -> [UInt8] {
    let w = image.width, h = image.height
    var bytes = [UInt8](repeating: 0, count: w*h*4)
    bytes.withUnsafeMutableBytes { buffer in
        let ctx = CGContext(data: buffer.baseAddress, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w*4, space: color, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue | CGBitmapInfo.byteOrder32Big.rawValue)!
        ctx.draw(image, in: CGRect(x:0,y:0,width:w,height:h))
    }
    // Store straight alpha; CGContext uses premultiplied alpha internally.
    for i in 0..<(w*h) { let a=Int(bytes[4*i+3]); if a>0 && a<255 { for c in 0..<3 {bytes[4*i+c]=UInt8(min(255,(Int(bytes[4*i+c])*255+a/2)/a))} } }
    return bytes
}
func maskBytes(_ image: CGImage) -> [UInt8] {
    var bytes = [UInt8](repeating: 0, count: image.width * image.height)
    bytes.withUnsafeMutableBytes { buffer in
        let ctx = CGContext(data: buffer.baseAddress, width:image.width,height:image.height,bitsPerComponent:8,bytesPerRow:image.width,space:CGColorSpaceCreateDeviceGray(),bitmapInfo:CGImageAlphaInfo.none.rawValue)!
        ctx.draw(image,in:CGRect(x:0,y:0,width:image.width,height:image.height))
    }
    return bytes
}
func segment(_ image: CGImage, selection: [Double]? = nil) throws -> ([UInt8], Int, Bool) {
    var error: NSError?
    guard let result = IAGSSegment(image, selection?.first ?? 0.5, selection?.last ?? 0.5, selection != nil, &error) else { throw error ?? fail("Vision foreground segmentation failed") }
    guard let bytes = result["bytes"] as? Data, let w = result["width"] as? Int, let h = result["height"] as? Int, w == image.width, h == image.height else { throw fail("Mask dimensions disagree") }
    return ([UInt8](bytes), result["instances"] as? Int ?? 1, result["fallback"] as? Bool ?? false)
}
func output(_ rgb: [UInt8], _ alpha: [UInt8], _ w: Int, _ h: Int, _ root: URL, _ id: String) throws -> Double {
    var rgba = rgb
    for i in 0..<(w * h) { rgba[4*i+3] = min(rgb[4*i+3], alpha[i]) }
    try savePNG(try cgImage(rgba,w,h), root.appendingPathComponent("output/\(id).png"))
    try savePNG(try cgImage(alpha,w,h,gray:true), root.appendingPathComponent("masks/\(id).png"))
    return Double(alpha.filter { $0 > 127 }.count) / Double(w*h)
}
func prepare(_ input: URL, _ root: URL, _ maxDimension: Int) throws {
    for folder in ["originals", "output", "masks", "previews", "base-masks", "logs"] { try fm.createDirectory(at: root.appendingPathComponent(folder), withIntermediateDirectories: true) }
    let files = try fm.contentsOfDirectory(at: input, includingPropertiesForKeys: [.isRegularFileKey]).filter { ["jpg","jpeg","png"].contains($0.pathExtension.lowercased()) && ((try? $0.resourceValues(forKeys: [.isRegularFileKey]).isRegularFile) == true) }.sorted { $0.lastPathComponent.localizedStandardCompare($1.lastPathComponent) == .orderedAscending }
    guard files.count >= 2 else { throw fail("至少需要两张 JPG/JPEG/PNG 照片，不支持视频") }
    let manifestURL = root.appendingPathComponent("processing.json")
    var seenHashes = Set<String>()
    var manifest = Manifest(maxDimension: maxDimension, name: input.lastPathComponent, photos: [], complete: false)
    try writeJSON(manifest, manifestURL)
    for (index, file) in files.enumerated() {
        try autoreleasepool {
            emit(["current": index, "total": files.count, "message": "正在处理 \(file.lastPathComponent)"])
            guard let source = CGImageSourceCreateWithURL(file as CFURL, nil), let props = CGImageSourceCopyPropertiesAtIndex(source,0,nil) as? [CFString:Any] else { throw fail("无法读取 \(file.lastPathComponent)") }
            let exif = props[kCGImagePropertyExifDictionary] as? [CFString:Any] ?? [:]
            let tiff = props[kCGImagePropertyTIFFDictionary] as? [CFString:Any] ?? [:]
            let options: [CFString:Any] = [kCGImageSourceCreateThumbnailFromImageAlways:true,kCGImageSourceCreateThumbnailWithTransform:true,kCGImageSourceThumbnailMaxPixelSize:maxDimension,kCGImageSourceShouldCacheImmediately:true]
            guard let image = CGImageSourceCreateThumbnailAtIndex(source,0,options as CFDictionary) else { throw fail("无法解码 \(file.lastPathComponent)") }
            let w=image.width, h=image.height, id=String(format:"frame_%06d",index+1)
            let rgb=rgbBytes(image)
            guard stride(from:3,to:rgb.count,by:4).contains(where:{rgb[$0]>0}) else {throw fail("图像解码没有返回可见像素。请检查图片或 macOS 图形服务权限。")}
            var alpha=(0..<(w*h)).map { rgb[$0*4+3] }
            var warnings:[String]=[]; var instances=1
            if !alpha.contains(where:{$0<255}) {
                do { let result=try segment(image); alpha=result.0; instances=result.1
                    if result.2 { warnings.append("中心不是主体，已选择最大前景；请检查") }
                    if instances>1 { warnings.append("检测到多个主体，可点击重新选择") }
                } catch { warnings.append("自动抠图失败：\(error.localizedDescription)；请手动修正或排除") }
            }
            let coverage=try output(rgb,alpha,w,h,root,id)
            if coverage < 0.015 || coverage > 0.95 { warnings.append("前景占比异常，请检查遮罩") }
            let focal35=(exif[kCGImagePropertyExifFocalLenIn35mmFilm] as? NSNumber)?.doubleValue
            let focal=(exif[kCGImagePropertyExifFocalLength] as? NSNumber)?.doubleValue
            let zoom=(exif[kCGImagePropertyExifDigitalZoomRatio] as? NSNumber)?.doubleValue
            let model=tiff[kCGImagePropertyTIFFModel] as? String ?? "unknown"
            let lens=exif[kCGImagePropertyExifLensModel] as? String ?? "unknown"
            let known = focal35 != nil && focal35! > 0 && focal != nil && focal! > 0 && model != "unknown"
            let key=known ? "\(model)|\(lens)|f=\(focal!)|eq=\(focal35!)|z=\(zoom ?? 1)|\(w)x\(h)" : "unknown-\(id)"
            if !known { warnings.append("缺少可靠焦距信息：使用独立相机，由重建估计") }
            let exposure=(exif[kCGImagePropertyExifExposureTime] as? NSNumber)?.doubleValue
            if let exposure, exposure > 0.04 { warnings.append("快门较慢，请检查运动模糊") }
            let originalDestination=root.appendingPathComponent("originals/\(id).\(file.pathExtension.lowercased())")
            try fm.copyItem(at:file,to:originalDestination)
            try savePNG(try cgImage(rgb,w,h),root.appendingPathComponent("previews/\(id).png"))
            try savePNG(try cgImage(alpha,w,h,gray:true),root.appendingPathComponent("base-masks/\(id).png"))
            let hash=SHA256.hash(data:try Data(contentsOf:file)).map {String(format:"%02x",$0)}.joined()
            if !seenHashes.insert(hash).inserted { warnings.append("与之前照片完全重复，建议排除") }
            // Gradient energy is a review hint only, never grounds for silently deleting a view.
            var energy=0.0;var samples=0
            for y in stride(from:1,to:h-1,by:4) { for x in stride(from:1,to:w-1,by:4) {
                let i=y*w+x
                if alpha[i]>127 {let center=Double(rgb[4*i+1]);let lap=Double(rgb[4*(i-1)+1])+Double(rgb[4*(i+1)+1])+Double(rgb[4*(i-w)+1])+Double(rgb[4*(i+w)+1])-4*center;energy+=lap*lap;samples+=1}
            }}
            if samples>0 && energy/Double(samples)<12 {warnings.append("清晰度或纹理偏低，请检查；该提示不会自动排除照片")}
            manifest.photos.append(Photo(id:id,originalName:file.lastPathComponent,width:w,height:h,originalWidth:props[kCGImagePropertyPixelWidth] as? Int ?? w,originalHeight:props[kCGImagePropertyPixelHeight] as? Int ?? h,cameraKey:key,focal35:focal35,digitalZoom:zoom,instances:instances,coverage:coverage,warnings:warnings,enabled:true,reviewed:false,sha256:hash))
            try writeJSON(manifest,manifestURL)
        }
    }
    manifest.complete=true; try writeJSON(manifest,manifestURL)
    emit(["current":files.count,"total":files.count,"message":"照片处理完成，请检查遮罩"])
}
func edit(_ root: URL, _ editURL: URL) throws {
    let request=try JSONDecoder().decode(Edit.self,from:Data(contentsOf:editURL))
    let manifestURL=root.appendingPathComponent("processing.json")
    var manifest=try JSONDecoder().decode(Manifest.self,from:Data(contentsOf:manifestURL))
    guard let index=manifest.photos.firstIndex(where:{$0.id==request.id}) else { throw fail("Unknown photo") }
    let photo=manifest.photos[index], w=photo.width, h=photo.height
    let image=try readCG(root.appendingPathComponent("previews/\(photo.id).png")); let rgb=rgbBytes(image)
    var alpha=maskBytes(try readCG(root.appendingPathComponent("masks/\(photo.id).png")))
    if let selected=request.select { alpha=try segment(image,selection:selected).0 }
    for stroke in request.strokes {
        guard stroke.radius.isFinite && stroke.radius>0 && stroke.radius<=0.25 && stroke.points.count<=20000 else { throw fail("Invalid brush stroke") }
        let radius=max(1,stroke.radius*Double(max(w,h)))
        var last:(Double,Double)?
        for p in stroke.points {
            guard p.count==2 && p.allSatisfy({$0.isFinite && $0>=0 && $0<=1}) else { throw fail("Invalid brush point") }
            let x=p[0]*Double(w), y=p[1]*Double(h)
            let previous=last ?? (x,y); let distance=hypot(x-previous.0,y-previous.1)
            let count=max(1,Int(ceil(distance/max(1,radius/3))))
            for step in 0...count {
                let f=Double(step)/Double(count), cx=previous.0+(x-previous.0)*f, cy=previous.1+(y-previous.1)*f
                let xmin=max(0,Int(cx-radius)), xmax=min(w-1,Int(cx+radius)), ymin=max(0,Int(cy-radius)), ymax=min(h-1,Int(cy+radius))
                if xmin<=xmax && ymin<=ymax { for yy in ymin...ymax { for xx in xmin...xmax {
                    if hypot(Double(xx)-cx,Double(yy)-cy)<=radius { alpha[yy*w+xx]=stroke.erase ? 0 : 255 }
                } } }
            }
            last=(x,y)
        }
    }
    manifest.photos[index].coverage=try output(rgb,alpha,w,h,root,photo.id)
    manifest.photos[index].enabled=request.enabled; manifest.photos[index].reviewed=request.reviewed
    try writeJSON(manifest,manifestURL)
}
do {
    let a=CommandLine.arguments
    guard a.count>=4 else { throw fail("Usage: iags-photo prepare INPUT OUTPUT [max=2400] | edit ROOT EDIT.json") }
    switch a[1] {
    case "prepare": let maxDim=a.count>4 ? (Int(a[4]) ?? 2400) : 2400; guard (512...4096).contains(maxDim) else { throw fail("Maximum dimension must be 512–4096") }; try prepare(URL(fileURLWithPath:a[2]),URL(fileURLWithPath:a[3]),maxDim)
    case "edit": try edit(URL(fileURLWithPath:a[2]),URL(fileURLWithPath:a[3]))
    default: throw fail("Unknown operation")
    }
} catch { fputs(error.localizedDescription+"\n",stderr); exit(1) }
