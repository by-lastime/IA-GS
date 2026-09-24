// IA'GS: narrow Objective-C bridge to the system Vision foreground-mask request.
#import "VisionBridge.h"
#import <Vision/VNGenerateForegroundInstanceMaskRequest.h>
#import <Vision/VNRequestHandler.h>
#import <CoreVideo/CoreVideo.h>
NSDictionary *IAGSSegment(CGImageRef image, double x, double y, BOOL explicitSelection, NSError **error) {
    VNImageRequestHandler *handler=[[VNImageRequestHandler alloc] initWithCGImage:image options:@{}];
    VNGenerateForegroundInstanceMaskRequest *request=[VNGenerateForegroundInstanceMaskRequest new];
    if(![handler performRequests:@[request] error:error])return nil;
    VNInstanceMaskObservation *result=request.results.firstObject;
    if(!result || result.allInstances.count==0){if(error)*error=[NSError errorWithDomain:@"IA'GS" code:1 userInfo:@{NSLocalizedDescriptionKey:@"未检测到前景，请手动修正或排除照片"}];return nil;}
    CVPixelBufferRef instance=result.instanceMask; CVPixelBufferLockBaseAddress(instance,kCVPixelBufferLock_ReadOnly);
    size_t w=CVPixelBufferGetWidth(instance),h=CVPixelBufferGetHeight(instance),stride=CVPixelBufferGetBytesPerRow(instance);
    const uint8_t *base=CVPixelBufferGetBaseAddress(instance);
    size_t px=MIN(w-1,(size_t)MAX(0,x*w)),py=MIN(h-1,(size_t)MAX(0,y*h));
    NSUInteger selected=base[py*stride+px];BOOL fallback=selected==0;
    if(fallback){size_t counts[256]={0};for(size_t yy=0;yy<h;yy++)for(size_t xx=0;xx<w;xx++)counts[base[yy*stride+xx]]++;size_t largest=0;for(NSUInteger i=1;i<256;i++)if(counts[i]>largest){selected=i;largest=counts[i];}}
    CVPixelBufferUnlockBaseAddress(instance,kCVPixelBufferLock_ReadOnly);
    if(explicitSelection&&fallback){if(error)*error=[NSError errorWithDomain:@"IA'GS" code:2 userInfo:@{NSLocalizedDescriptionKey:@"点击位置是背景，请点在目标文物内部"}];return nil;}
    CVPixelBufferRef mask=[result generateScaledMaskForImageForInstances:[NSIndexSet indexSetWithIndex:selected] fromRequestHandler:handler error:error];
    if(!mask)return nil;
    CVPixelBufferLockBaseAddress(mask,kCVPixelBufferLock_ReadOnly);
    size_t mw=CVPixelBufferGetWidth(mask),mh=CVPixelBufferGetHeight(mask),row=CVPixelBufferGetBytesPerRow(mask);const uint8_t *data=CVPixelBufferGetBaseAddress(mask);
    NSMutableData *alpha=[NSMutableData dataWithLength:mw*mh];uint8_t *out=alpha.mutableBytes;
    OSType format=CVPixelBufferGetPixelFormatType(mask);
    if(format!=kCVPixelFormatType_OneComponent32Float && format!=kCVPixelFormatType_OneComponent8) {
        CVPixelBufferUnlockBaseAddress(mask,kCVPixelBufferLock_ReadOnly);CVPixelBufferRelease(mask);
        if(error)*error=[NSError errorWithDomain:@"IA'GS" code:3 userInfo:@{NSLocalizedDescriptionKey:@"Unsupported Vision mask pixel format"}];return nil;
    }
    for(size_t yy=0;yy<mh;yy++)for(size_t xx=0;xx<mw;xx++){
        if(format==kCVPixelFormatType_OneComponent32Float){float value=((const float *)(data+yy*row))[xx];out[yy*mw+xx]=(uint8_t)lroundf(fmaxf(0,fminf(255,value*255)));}
        else out[yy*mw+xx]=data[yy*row+xx];
    }
    CVPixelBufferUnlockBaseAddress(mask,kCVPixelBufferLock_ReadOnly);
    CVPixelBufferRelease(mask);
    return @{ @"bytes":alpha,@"width":@(mw),@"height":@(mh),@"instances":@(result.allInstances.count),@"fallback":@(fallback) };
}
