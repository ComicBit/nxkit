// archive_probe <script.mtl4-json> <archive.mtl4a>...: opens the archives with -[MTLDevice newArchiveWithURL:] and looks up
// every render pipeline of the script in them as the host does (pre_desc in nvn_mtl.m: library function descriptors,
// empty vertex descriptor, sample count 1, colour state unspecialized). Prints "found <n> of <total>", exit 0 when all
// are found. The OS-level check of "equal" for archives that are not byte-identical (nxb-archive).
// Build: xcrun clang -fobjc-arc -framework Metal -framework Foundation archive_probe.m -o archive_probe
#import <Foundation/Foundation.h>
#import <Metal/Metal.h>

int main(int argc, char **argv) { @autoreleasepool {
    if (argc < 3) { fprintf(stderr, "usage: archive_probe <script.mtl4-json> <archive.mtl4a>...\n"); return 2; }
    id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
    NSData *js = [NSData dataWithContentsOfFile:@(argv[1])];
    NSDictionary *s = js ? [NSJSONSerialization JSONObjectWithData:js options:0 error:nil] : nil;
    if (!s) { fprintf(stderr, "cannot read %s\n", argv[1]); return 2; }
    NSMutableArray<id<MTL4Archive>> *archives = [NSMutableArray array];
    for (int i = 2; i < argc; i++) {
        NSError *e = nil; id<MTL4Archive> a = [dev newArchiveWithURL:[NSURL fileURLWithPath:@(argv[i])] error:&e];
        if (!a) { fprintf(stderr, "archive %s not opened: %s\n", argv[i], e.localizedDescription.UTF8String); return 1; }
        [archives addObject:a];
    }
    NSMutableDictionary<NSString *, id<MTLLibrary>> *libs = [NSMutableDictionary dictionary];
    for (NSDictionary *l in s[@"libraries"]) {
        NSError *e = nil; id<MTLLibrary> lib = [dev newLibraryWithURL:[NSURL fileURLWithPath:l[@"path"]] error:&e];
        if (!lib) { fprintf(stderr, "library %s: %s\n", [l[@"path"] UTF8String], e.localizedDescription.UTF8String); return 1; }
        libs[[@"lib:" stringByAppendingString:l[@"label"]]] = lib;
    }
    NSMutableDictionary<NSString *, MTL4LibraryFunctionDescriptor *> *fns = [NSMutableDictionary dictionary];
    for (NSDictionary *f in s[@"function_descriptors"][@"library_function_descriptors"]) {
        MTL4LibraryFunctionDescriptor *d = [MTL4LibraryFunctionDescriptor new]; d.library = libs[f[@"library"]]; d.name = f[@"name"];
        fns[[@"fnd:" stringByAppendingString:f[@"label"]]] = d;
    }
    NSArray *pipes = s[@"pipeline_descriptors"][@"render_pipeline_descriptors"];
    long found = 0, n = 0;
    for (NSDictionary *p in pipes) { @autoreleasepool {
        n++;
        MTL4RenderPipelineDescriptor *m = [MTL4RenderPipelineDescriptor new];
        m.vertexFunctionDescriptor = fns[p[@"vertex_function_descriptor"]]; m.fragmentFunctionDescriptor = fns[p[@"fragment_function_descriptor"]];
        m.vertexDescriptor = [MTLVertexDescriptor vertexDescriptor]; m.rasterSampleCount = 1;
        for (int i = 0; i < 8; i++) { MTL4RenderPipelineColorAttachmentDescriptor *a = m.colorAttachments[i];
            a.pixelFormat = MTLPixelFormatUnspecialized; a.writeMask = MTLColorWriteMaskUnspecialized; a.blendingState = MTL4BlendStateUnspecialized;
            a.sourceRGBBlendFactor = a.destinationRGBBlendFactor = a.sourceAlphaBlendFactor = a.destinationAlphaBlendFactor = MTLBlendFactorUnspecialized;
            a.rgbBlendOperation = a.alphaBlendOperation = MTLBlendOperationUnspecialized; }
        for (id<MTL4Archive> a in archives) if ([a newRenderPipelineStateWithDescriptor:m error:nil]) { found++; break; }
    } }
    printf("found %ld of %ld\n", found, n);
    return found == n ? 0 : 1;
} }
