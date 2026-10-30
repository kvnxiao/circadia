#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color {
    pub a: u8,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}
impl windows_core::imp::TypeKind for Color {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for Color {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::from_slice(b"struct(Windows.UI.Color;u1;u1;u1;u1)");
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ColorKeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ColorKeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    ColorKeyFrameAnimation,
    KeyFrameAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for ColorKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IColorKeyFrameAnimation>();
}
unsafe impl windows_core::Interface for ColorKeyFrameAnimation {
    type Vtable = <IColorKeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IColorKeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for ColorKeyFrameAnimation {
    type Target = IColorKeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ColorKeyFrameAnimation {
    const NAME: &'static str = "Microsoft.UI.Composition.ColorKeyFrameAnimation";
}
unsafe impl Send for ColorKeyFrameAnimation {}
unsafe impl Sync for ColorKeyFrameAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionAnimation, CompositionObject);
impl windows_core::RuntimeType for CompositionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionAnimation>();
}
unsafe impl windows_core::Interface for CompositionAnimation {
    type Vtable = <ICompositionAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionAnimation {
    type Target = ICompositionAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionAnimation {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionAnimation";
}
unsafe impl Send for CompositionAnimation {}
unsafe impl Sync for CompositionAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionBrush, CompositionObject);
impl windows_core::RuntimeType for CompositionBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionBrush>();
}
unsafe impl windows_core::Interface for CompositionBrush {
    type Vtable = <ICompositionBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionBrush {
    type Target = ICompositionBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionBrush {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionBrush";
}
unsafe impl Send for CompositionBrush {}
unsafe impl Sync for CompositionBrush {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionColorBrush(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionColorBrush,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionColorBrush, CompositionBrush, CompositionObject);
impl windows_core::RuntimeType for CompositionColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionColorBrush>();
}
unsafe impl windows_core::Interface for CompositionColorBrush {
    type Vtable = <ICompositionColorBrush as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionColorBrush as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionColorBrush {
    type Target = ICompositionColorBrush;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionColorBrush {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionColorBrush";
}
unsafe impl Send for CompositionColorBrush {}
unsafe impl Sync for CompositionColorBrush {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionColorSpace(pub i32);
impl CompositionColorSpace {
    pub const Auto: Self = Self(0);
    pub const Hsl: Self = Self(1);
    pub const Rgb: Self = Self(2);
    pub const HslLinear: Self = Self(3);
    pub const RgbLinear: Self = Self(4);
}
impl windows_core::imp::TypeKind for CompositionColorSpace {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for CompositionColorSpace {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Microsoft.UI.Composition.CompositionColorSpace;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEasingFunction(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEasingFunction,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionEasingFunction, CompositionObject);
impl windows_core::RuntimeType for CompositionEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEasingFunction>();
}
unsafe impl windows_core::Interface for CompositionEasingFunction {
    type Vtable = <ICompositionEasingFunction as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionEasingFunction as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionEasingFunction {
    type Target = ICompositionEasingFunction;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionEasingFunction {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionEasingFunction";
}
unsafe impl Send for CompositionEasingFunction {}
unsafe impl Sync for CompositionEasingFunction {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionEllipseGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionEllipseGeometry,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    CompositionEllipseGeometry,
    CompositionGeometry,
    CompositionObject
);
impl windows_core::RuntimeType for CompositionEllipseGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionEllipseGeometry>();
}
unsafe impl windows_core::Interface for CompositionEllipseGeometry {
    type Vtable = <ICompositionEllipseGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionEllipseGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionEllipseGeometry {
    type Target = ICompositionEllipseGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionEllipseGeometry {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionEllipseGeometry";
}
unsafe impl Send for CompositionEllipseGeometry {}
unsafe impl Sync for CompositionEllipseGeometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionGeometry(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionGeometry,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionGeometry, CompositionObject);
impl windows_core::RuntimeType for CompositionGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionGeometry>();
}
unsafe impl windows_core::Interface for CompositionGeometry {
    type Vtable = <ICompositionGeometry as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionGeometry as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionGeometry {
    type Target = ICompositionGeometry;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionGeometry {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionGeometry";
}
unsafe impl Send for CompositionGeometry {}
unsafe impl Sync for CompositionGeometry {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionObject(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionObject,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for CompositionObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionObject>();
}
unsafe impl windows_core::Interface for CompositionObject {
    type Vtable = <ICompositionObject as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionObject as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionObject {
    type Target = ICompositionObject;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionObject {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionObject";
}
unsafe impl Send for CompositionObject {}
unsafe impl Sync for CompositionObject {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionShape(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionShape,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionShape, CompositionObject);
impl windows_core::RuntimeType for CompositionShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionShape>();
}
unsafe impl windows_core::Interface for CompositionShape {
    type Vtable = <ICompositionShape as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionShape as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionShape {
    type Target = ICompositionShape;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionShape {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionShape";
}
unsafe impl Send for CompositionShape {}
unsafe impl Sync for CompositionShape {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionShapeCollection(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionShapeCollection,
    windows_core::IUnknown,
    windows_core::IInspectable,
    windows_collections::IVector<CompositionShape>
);
windows_core::imp::required_hierarchy!(CompositionShapeCollection, CompositionObject);
impl windows_core::RuntimeType for CompositionShapeCollection {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::for_class::<
        Self,
        windows_collections::IVector<CompositionShape>,
    >();
}
unsafe impl windows_core::Interface for CompositionShapeCollection {
    type Vtable =
        <windows_collections::IVector<CompositionShape> as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID =
        <windows_collections::IVector<CompositionShape> as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionShapeCollection {
    type Target = windows_collections::IVector<CompositionShape>;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionShapeCollection {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionShapeCollection";
}
unsafe impl Send for CompositionShapeCollection {}
unsafe impl Sync for CompositionShapeCollection {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionSpriteShape(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionSpriteShape,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(CompositionSpriteShape, CompositionShape, CompositionObject);
impl windows_core::RuntimeType for CompositionSpriteShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionSpriteShape>();
}
unsafe impl windows_core::Interface for CompositionSpriteShape {
    type Vtable = <ICompositionSpriteShape as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionSpriteShape as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionSpriteShape {
    type Target = ICompositionSpriteShape;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionSpriteShape {
    const NAME: &'static str = "Microsoft.UI.Composition.CompositionSpriteShape";
}
unsafe impl Send for CompositionSpriteShape {}
unsafe impl Sync for CompositionSpriteShape {}
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CompositionStrokeCap(pub i32);
impl CompositionStrokeCap {
    pub const Flat: Self = Self(0);
    pub const Square: Self = Self(1);
    pub const Round: Self = Self(2);
    pub const Triangle: Self = Self(3);
}
impl windows_core::imp::TypeKind for CompositionStrokeCap {
    type TypeKind = windows_core::imp::CopyType;
}
impl windows_core::RuntimeType for CompositionStrokeCap {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::from_slice(
        b"enum(Microsoft.UI.Composition.CompositionStrokeCap;i4)",
    );
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionTarget(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    CompositionTarget,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl CompositionTarget {
    pub fn Rendering<F>(handler: F) -> windows_core::Result<windows_core::EventRevoker>
    where
        F: Fn(
                windows_core::Ref<windows_core::IInspectable>,
                windows_core::Ref<windows_core::IInspectable>,
            ) + 'static,
    {
        let handler: EventHandler<windows_core::IInspectable> = {
            let com =
                windows_core::imp::DelegateBox::<EventHandler<windows_core::IInspectable>, F>::new(
                    &EventHandlerBox::<windows_core::IInspectable, F>::VTABLE,
                    handler,
                );
            unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
        };
        Self::ICompositionTargetStatics(|this| unsafe {
            let mut result__ = core::mem::zeroed();
            let token__ = (windows_core::Interface::vtable(this).Rendering)(
                windows_core::Interface::as_raw(this),
                windows_core::Interface::as_raw(&handler),
                &mut result__,
            )
            .map(|| result__)?;
            Ok(windows_core::EventRevoker::new(
                this.clone(),
                token__,
                windows_core::Interface::vtable(this).RemoveRendering,
            ))
        })
    }
    fn ICompositionTargetStatics<
        R,
        F: FnOnce(&ICompositionTargetStatics) -> windows_core::Result<R>,
    >(
        callback: F,
    ) -> windows_core::Result<R> {
        static SHARED: windows_core::imp::FactoryCache<
            CompositionTarget,
            ICompositionTargetStatics,
        > = windows_core::imp::FactoryCache::new();
        SHARED.call(callback)
    }
}
impl windows_core::RuntimeType for CompositionTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositionTarget>();
}
unsafe impl windows_core::Interface for CompositionTarget {
    type Vtable = <ICompositionTarget as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositionTarget as windows_core::Interface>::IID;
}
impl core::ops::Deref for CompositionTarget {
    type Target = ICompositionTarget;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for CompositionTarget {
    const NAME: &'static str = "Microsoft.UI.Xaml.Media.CompositionTarget";
}
unsafe impl Send for CompositionTarget {}
unsafe impl Sync for CompositionTarget {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compositor(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    Compositor,
    windows_core::IUnknown,
    windows_core::IInspectable
);
impl windows_core::RuntimeType for Compositor {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ICompositor>();
}
unsafe impl windows_core::Interface for Compositor {
    type Vtable = <ICompositor as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ICompositor as windows_core::Interface>::IID;
}
impl core::ops::Deref for Compositor {
    type Target = ICompositor;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Compositor {
    const NAME: &'static str = "Microsoft.UI.Composition.Compositor";
}
unsafe impl Send for Compositor {}
unsafe impl Sync for Compositor {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainerVisual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ContainerVisual,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(ContainerVisual, Visual, CompositionObject);
impl windows_core::RuntimeType for ContainerVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IContainerVisual>();
}
unsafe impl windows_core::Interface for ContainerVisual {
    type Vtable = <IContainerVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IContainerVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for ContainerVisual {
    type Target = IContainerVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ContainerVisual {
    const NAME: &'static str = "Microsoft.UI.Composition.ContainerVisual";
}
unsafe impl Send for ContainerVisual {}
unsafe impl Sync for ContainerVisual {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventHandler<T>(windows_core::IUnknown, core::marker::PhantomData<T>)
where
    T: windows_core::RuntimeType + 'static;
unsafe impl<T: windows_core::RuntimeType + 'static> windows_core::Interface for EventHandler<T> {
    type Vtable = EventHandler_Vtbl<T>;
    const IID: windows_core::GUID =
        windows_core::GUID::from_signature(<Self as windows_core::RuntimeType>::SIGNATURE);
}
impl<T: windows_core::RuntimeType + 'static> windows_core::RuntimeType for EventHandler<T> {
    const SIGNATURE: windows_core::imp::ConstBuffer = windows_core::imp::ConstBuffer::new()
        .push_slice(b"pinterface({9de1c535-6ae1-11e0-84e1-18a905bcc53f}")
        .push_slice(b";")
        .push_other(T::SIGNATURE)
        .push_slice(b")");
}
#[repr(C)]
pub struct EventHandler_Vtbl<T>
where
    T: windows_core::RuntimeType + 'static,
{
    base__: windows_core::IUnknown_Vtbl,
    Invoke: unsafe extern "system" fn(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        args: windows_core::imp::AbiType<T>,
    ) -> windows_core::HRESULT,
    T: core::marker::PhantomData<T>,
}
struct EventHandlerBox<
    T,
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<T>) + 'static,
>(core::marker::PhantomData<(T, fn() -> F)>)
where
    T: windows_core::RuntimeType + 'static;
impl<
    T: windows_core::RuntimeType + 'static,
    F: Fn(windows_core::Ref<windows_core::IInspectable>, windows_core::Ref<T>) + 'static,
> EventHandlerBox<T, F>
{
    const VTABLE: EventHandler_Vtbl<T> = EventHandler_Vtbl::<T> {
        base__: windows_core::IUnknown_Vtbl {
            QueryInterface: windows_core::imp::DelegateBox::<EventHandler<T>, F>::QueryInterface,
            AddRef: windows_core::imp::DelegateBox::<EventHandler<T>, F>::AddRef,
            Release: windows_core::imp::DelegateBox::<EventHandler<T>, F>::Release,
        },
        Invoke: Self::Invoke,
        T: core::marker::PhantomData::<T>,
    };
    unsafe extern "system" fn Invoke(
        this: *mut core::ffi::c_void,
        sender: *mut core::ffi::c_void,
        args: windows_core::imp::AbiType<T>,
    ) -> windows_core::HRESULT {
        unsafe {
            let this = &mut *(this as *mut *mut core::ffi::c_void
                as *mut windows_core::imp::DelegateBox<EventHandler<T>, F>);
            (this.invoke)(
                core::mem::transmute_copy(&sender),
                core::mem::transmute_copy(&args),
            );
            windows_core::HRESULT(0)
        }
    }
}
windows_core::imp::define_interface!(
    IColorKeyFrameAnimation,
    IColorKeyFrameAnimation_Vtbl,
    0xf0237928_353c_5867_be93_71547e989f44
);
impl windows_core::RuntimeType for IColorKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IColorKeyFrameAnimation {
    pub fn SetInterpolationColorSpace(
        &self,
        value: CompositionColorSpace,
    ) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetInterpolationColorSpace)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn InsertKeyFrameWithEasingFunction<P2>(
        &self,
        normalizedprogresskey: f32,
        value: Color,
        easingfunction: P2,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<CompositionEasingFunction>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrameWithEasingFunction)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
                easingfunction.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IColorKeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    InterpolationColorSpace: usize,
    pub SetInterpolationColorSpace: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionColorSpace,
    ) -> windows_core::HRESULT,
    InsertKeyFrame: usize,
    pub InsertKeyFrameWithEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        Color,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionAnimation,
    ICompositionAnimation_Vtbl,
    0xa829ccc8_6fde_5b90_ad37_efd307e1b631
);
impl windows_core::RuntimeType for ICompositionAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionBrush,
    ICompositionBrush_Vtbl,
    0x483924e7_99a5_5377_968b_dec6d40bbccd
);
impl windows_core::RuntimeType for ICompositionBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionColorBrush,
    ICompositionColorBrush_Vtbl,
    0x3f8ffb69_3e71_55a7_8e79_f27a214c56ae
);
impl windows_core::RuntimeType for ICompositionColorBrush {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionColorBrush {
    pub fn Color(&self) -> windows_core::Result<Color> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Color)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .map(|| result__)
        }
    }
    pub fn SetColor(&self, value: Color) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetColor)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionColorBrush_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Color:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut Color) -> windows_core::HRESULT,
    pub SetColor: unsafe extern "system" fn(*mut core::ffi::c_void, Color) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionEasingFunction,
    ICompositionEasingFunction_Vtbl,
    0x8e1ecd0d_57d8_5bc9_9bcd_e43d0dd733c4
);
impl windows_core::RuntimeType for ICompositionEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionEasingFunction_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionEllipseGeometry,
    ICompositionEllipseGeometry_Vtbl,
    0xf2a21042_7a57_58c1_8b47_8bc8b21d3aa0
);
impl windows_core::RuntimeType for ICompositionEllipseGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionEllipseGeometry {
    pub fn SetCenter(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCenter)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetRadius(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRadius)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionEllipseGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Center: usize,
    pub SetCenter: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Radius: usize,
    pub SetRadius: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionGeometry,
    ICompositionGeometry_Vtbl,
    0x4e40bdb2_450b_5a81_9e9b_149417980cc4
);
impl windows_core::RuntimeType for ICompositionGeometry {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionGeometry {
    pub fn SetTrimEnd(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTrimEnd)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTrimOffset(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTrimOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetTrimStart(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetTrimStart)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionGeometry_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    TrimEnd: usize,
    pub SetTrimEnd: unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    TrimOffset: usize,
    pub SetTrimOffset:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
    TrimStart: usize,
    pub SetTrimStart:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionObject,
    ICompositionObject_Vtbl,
    0x0e583d49_fb5e_5481_a426_d3c41e059a5a
);
impl windows_core::RuntimeType for ICompositionObject {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionObject {
    pub fn Compositor(&self) -> windows_core::Result<Compositor> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Compositor)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn StartAnimation<P1>(&self, propertyname: &str, animation: P1) -> windows_core::Result<()>
    where
        P1: windows_core::Param<CompositionAnimation>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).StartAnimation)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
                animation.param().abi(),
            )
            .ok()
        }
    }
    pub fn StopAnimation(&self, propertyname: &str) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).StopAnimation)(
                windows_core::Interface::as_raw(self),
                core::mem::transmute_copy(&windows_core::HSTRING::from(propertyname)),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionObject_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Compositor: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Properties: usize,
    pub StartAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub StopAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionShape,
    ICompositionShape_Vtbl,
    0xed75d4d8_437f_5640_9720_faae35ce5895
);
impl windows_core::RuntimeType for ICompositionShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionShape {
    pub fn SetCenterPoint(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetCenterPoint)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetOffset(&self, value: windows_numerics::Vector2) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetOffset)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetRotationAngleInDegrees(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetRotationAngleInDegrees)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionShape_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    CenterPoint: usize,
    pub SetCenterPoint: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    Offset: usize,
    pub SetOffset: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_numerics::Vector2,
    ) -> windows_core::HRESULT,
    RotationAngle: usize,
    SetRotationAngle: usize,
    RotationAngleInDegrees: usize,
    pub SetRotationAngleInDegrees:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionSpriteShape,
    ICompositionSpriteShape_Vtbl,
    0x982138f2_5781_509a_ba5d_112bcb0b98ef
);
impl windows_core::RuntimeType for ICompositionSpriteShape {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositionSpriteShape {
    pub fn FillBrush(&self) -> windows_core::Result<CompositionBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).FillBrush)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn SetFillBrush<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetFillBrush)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetStrokeBrush<P0>(&self, value: P0) -> windows_core::Result<()>
    where
        P0: windows_core::Param<CompositionBrush>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeBrush)(
                windows_core::Interface::as_raw(self),
                value.param().abi(),
            )
            .ok()
        }
    }
    pub fn SetStrokeEndCap(&self, value: CompositionStrokeCap) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeEndCap)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetStrokeStartCap(&self, value: CompositionStrokeCap) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeStartCap)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
    pub fn SetStrokeThickness(&self, value: f32) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetStrokeThickness)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct ICompositionSpriteShape_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub FillBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetFillBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    Geometry: usize,
    SetGeometry: usize,
    IsStrokeNonScaling: usize,
    SetIsStrokeNonScaling: usize,
    StrokeBrush: usize,
    pub SetStrokeBrush: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    StrokeDashArray: usize,
    StrokeDashCap: usize,
    SetStrokeDashCap: usize,
    StrokeDashOffset: usize,
    SetStrokeDashOffset: usize,
    StrokeEndCap: usize,
    pub SetStrokeEndCap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeCap,
    ) -> windows_core::HRESULT,
    StrokeLineJoin: usize,
    SetStrokeLineJoin: usize,
    StrokeMiterLimit: usize,
    SetStrokeMiterLimit: usize,
    StrokeStartCap: usize,
    pub SetStrokeStartCap: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        CompositionStrokeCap,
    ) -> windows_core::HRESULT,
    StrokeThickness: usize,
    pub SetStrokeThickness:
        unsafe extern "system" fn(*mut core::ffi::c_void, f32) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositionTarget,
    ICompositionTarget_Vtbl,
    0x7d938324_e3ad_597c_93f6_520725410e68
);
impl windows_core::RuntimeType for ICompositionTarget {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionTarget_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    ICompositionTargetStatics,
    ICompositionTargetStatics_Vtbl,
    0x12a4be6f_6db1_5165_b622_d57ab782745b
);
impl windows_core::RuntimeType for ICompositionTargetStatics {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ICompositionTargetStatics_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Rendering: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut i64,
    ) -> windows_core::HRESULT,
    pub RemoveRendering:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor,
    ICompositor_Vtbl,
    0x95213c13_c4cb_57de_b267_d21ab901ae38
);
impl windows_core::RuntimeType for ICompositor {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor {
    pub fn CreateColorKeyFrameAnimation(&self) -> windows_core::Result<ColorKeyFrameAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateColorKeyFrameAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn CreateColorBrushWithColor(
        &self,
        color: Color,
    ) -> windows_core::Result<CompositionColorBrush> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateColorBrushWithColor)(
                windows_core::Interface::as_raw(self),
                color,
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn CreateLinearEasingFunction(&self) -> windows_core::Result<LinearEasingFunction> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateLinearEasingFunction)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn CreateScalarKeyFrameAnimation(&self) -> windows_core::Result<ScalarKeyFrameAnimation> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateScalarKeyFrameAnimation)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub CreateColorKeyFrameAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateColorBrush: usize,
    pub CreateColorBrushWithColor: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        Color,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateContainerVisual: usize,
    CreateCubicBezierEasingFunction: usize,
    CreateEffectFactory: usize,
    CreateEffectFactoryWithProperties: usize,
    CreateExpressionAnimation: usize,
    CreateExpressionAnimationWithExpression: usize,
    CreateInsetClip: usize,
    CreateInsetClipWithInsets: usize,
    pub CreateLinearEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreatePropertySet: usize,
    CreateQuaternionKeyFrameAnimation: usize,
    pub CreateScalarKeyFrameAnimation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ICompositor5,
    ICompositor5_Vtbl,
    0xbb52d644_a030_5c19_b883_577ded739ae7
);
impl windows_core::RuntimeType for ICompositor5 {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl ICompositor5 {
    pub fn CreateEllipseGeometry(&self) -> windows_core::Result<CompositionEllipseGeometry> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateEllipseGeometry)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
    pub fn CreateSpriteShapeWithGeometry<P0>(
        &self,
        geometry: P0,
    ) -> windows_core::Result<CompositionSpriteShape>
    where
        P0: windows_core::Param<CompositionGeometry>,
    {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).CreateSpriteShapeWithGeometry)(
                windows_core::Interface::as_raw(self),
                geometry.param().abi(),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct ICompositor5_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    Comment: usize,
    SetComment: usize,
    GlobalPlaybackRate: usize,
    SetGlobalPlaybackRate: usize,
    CreateBounceScalarAnimation: usize,
    CreateBounceVector2Animation: usize,
    CreateBounceVector3Animation: usize,
    CreateContainerShape: usize,
    pub CreateEllipseGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    CreateLineGeometry: usize,
    CreatePathGeometry: usize,
    CreatePathGeometryWithPath: usize,
    CreatePathKeyFrameAnimation: usize,
    CreateRectangleGeometry: usize,
    CreateRoundedRectangleGeometry: usize,
    CreateShapeVisual: usize,
    CreateSpriteShape: usize,
    pub CreateSpriteShapeWithGeometry: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IContainerVisual,
    IContainerVisual_Vtbl,
    0xc70dbce1_2c2f_5d8e_91a4_aae1121e6186
);
impl windows_core::RuntimeType for IContainerVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IContainerVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IKeyFrameAnimation,
    IKeyFrameAnimation_Vtbl,
    0x5a8f57f0_f059_5b47_b308_c4c80fc71248
);
impl windows_core::RuntimeType for IKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IKeyFrameAnimation {
    pub fn SetDuration(&self, value: windows_time::TimeSpan) -> windows_core::Result<()> {
        unsafe {
            (windows_core::Interface::vtable(self).SetDuration)(
                windows_core::Interface::as_raw(self),
                value,
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IKeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    DelayTime: usize,
    SetDelayTime: usize,
    Duration: usize,
    pub SetDuration: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_time::TimeSpan,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    ILinearEasingFunction,
    ILinearEasingFunction_Vtbl,
    0x79bfeef6_70c7_50a6_bb3a_0e9636148695
);
impl windows_core::RuntimeType for ILinearEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct ILinearEasingFunction_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
windows_core::imp::define_interface!(
    IScalarKeyFrameAnimation,
    IScalarKeyFrameAnimation_Vtbl,
    0x5a5f8abe_d129_5b25_8aff_8180fd9bfb22
);
impl windows_core::RuntimeType for IScalarKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IScalarKeyFrameAnimation {
    pub fn InsertKeyFrameWithEasingFunction<P2>(
        &self,
        normalizedprogresskey: f32,
        value: f32,
        easingfunction: P2,
    ) -> windows_core::Result<()>
    where
        P2: windows_core::Param<CompositionEasingFunction>,
    {
        unsafe {
            (windows_core::Interface::vtable(self).InsertKeyFrameWithEasingFunction)(
                windows_core::Interface::as_raw(self),
                normalizedprogresskey,
                value,
                easingfunction.param().abi(),
            )
            .ok()
        }
    }
}
#[repr(C)]
pub struct IScalarKeyFrameAnimation_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    InsertKeyFrame: usize,
    pub InsertKeyFrameWithEasingFunction: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        f32,
        f32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IShapeVisual,
    IShapeVisual_Vtbl,
    0xa911c80b_a5a5_5aca_b8ff_c43f08f06143
);
impl windows_core::RuntimeType for IShapeVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
impl IShapeVisual {
    pub fn Shapes(&self) -> windows_core::Result<CompositionShapeCollection> {
        unsafe {
            let mut result__ = core::mem::zeroed();
            (windows_core::Interface::vtable(self).Shapes)(
                windows_core::Interface::as_raw(self),
                &mut result__,
            )
            .and_then(|| windows_core::imp::Type::from_abi(result__))
        }
    }
}
#[repr(C)]
pub struct IShapeVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
    pub Shapes: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IVisual,
    IVisual_Vtbl,
    0xc0eeab6c_c897_5ac6_a1c9_63abd5055b9b
);
impl windows_core::RuntimeType for IVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_interface::<Self>();
}
#[repr(C)]
pub struct IVisual_Vtbl {
    pub base__: windows_core::IInspectable_Vtbl,
}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    KeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(KeyFrameAnimation, CompositionAnimation, CompositionObject);
impl windows_core::RuntimeType for KeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IKeyFrameAnimation>();
}
unsafe impl windows_core::Interface for KeyFrameAnimation {
    type Vtable = <IKeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IKeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for KeyFrameAnimation {
    type Target = IKeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for KeyFrameAnimation {
    const NAME: &'static str = "Microsoft.UI.Composition.KeyFrameAnimation";
}
unsafe impl Send for KeyFrameAnimation {}
unsafe impl Sync for KeyFrameAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinearEasingFunction(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    LinearEasingFunction,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    LinearEasingFunction,
    CompositionEasingFunction,
    CompositionObject
);
impl windows_core::RuntimeType for LinearEasingFunction {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, ILinearEasingFunction>();
}
unsafe impl windows_core::Interface for LinearEasingFunction {
    type Vtable = <ILinearEasingFunction as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <ILinearEasingFunction as windows_core::Interface>::IID;
}
impl core::ops::Deref for LinearEasingFunction {
    type Target = ILinearEasingFunction;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for LinearEasingFunction {
    const NAME: &'static str = "Microsoft.UI.Composition.LinearEasingFunction";
}
unsafe impl Send for LinearEasingFunction {}
unsafe impl Sync for LinearEasingFunction {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScalarKeyFrameAnimation(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ScalarKeyFrameAnimation,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(
    ScalarKeyFrameAnimation,
    KeyFrameAnimation,
    CompositionAnimation,
    CompositionObject
);
impl windows_core::RuntimeType for ScalarKeyFrameAnimation {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IScalarKeyFrameAnimation>();
}
unsafe impl windows_core::Interface for ScalarKeyFrameAnimation {
    type Vtable = <IScalarKeyFrameAnimation as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IScalarKeyFrameAnimation as windows_core::Interface>::IID;
}
impl core::ops::Deref for ScalarKeyFrameAnimation {
    type Target = IScalarKeyFrameAnimation;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ScalarKeyFrameAnimation {
    const NAME: &'static str = "Microsoft.UI.Composition.ScalarKeyFrameAnimation";
}
unsafe impl Send for ScalarKeyFrameAnimation {}
unsafe impl Sync for ScalarKeyFrameAnimation {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ShapeVisual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(
    ShapeVisual,
    windows_core::IUnknown,
    windows_core::IInspectable
);
windows_core::imp::required_hierarchy!(ShapeVisual, ContainerVisual, Visual, CompositionObject);
impl windows_core::RuntimeType for ShapeVisual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IShapeVisual>();
}
unsafe impl windows_core::Interface for ShapeVisual {
    type Vtable = <IShapeVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IShapeVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for ShapeVisual {
    type Target = IShapeVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for ShapeVisual {
    const NAME: &'static str = "Microsoft.UI.Composition.ShapeVisual";
}
unsafe impl Send for ShapeVisual {}
unsafe impl Sync for ShapeVisual {}
#[repr(transparent)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Visual(windows_core::IUnknown);
windows_core::imp::interface_hierarchy!(Visual, windows_core::IUnknown, windows_core::IInspectable);
windows_core::imp::required_hierarchy!(Visual, CompositionObject);
impl windows_core::RuntimeType for Visual {
    const SIGNATURE: windows_core::imp::ConstBuffer =
        windows_core::imp::ConstBuffer::for_class::<Self, IVisual>();
}
unsafe impl windows_core::Interface for Visual {
    type Vtable = <IVisual as windows_core::Interface>::Vtable;
    const IID: windows_core::GUID = <IVisual as windows_core::Interface>::IID;
}
impl core::ops::Deref for Visual {
    type Target = IVisual;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
impl windows_core::RuntimeName for Visual {
    const NAME: &'static str = "Microsoft.UI.Composition.Visual";
}
unsafe impl Send for Visual {}
unsafe impl Sync for Visual {}
