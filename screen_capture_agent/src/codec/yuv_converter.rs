use scrap::ARGBToI420;

macro_rules! call_yuv {
    ($x:expr) => {{
        let result = unsafe { $x };
        if result != 0 {
            return Err(format!("YUV conversion failed with code: {}", result).into());
        }
    }};
}

pub struct YuvConverter {
    width: usize,
    height: usize,
    yuv_buffer: Vec<u8>,
}

impl YuvConverter {
    pub fn new(width: usize, height: usize) -> Result<Self, Box<dyn std::error::Error>> {
        if width == 0 || height == 0 {
            return Err("Width and height must be > 0".into());
        }
        let y_size = width * height;
        let uv_size = (width / 2) * (height / 2);
        let yuv_size = y_size + uv_size * 2;
        let align = |x: usize| (x + 63) / 64 * 64;
        let aligned_buffer_size = align(yuv_size);

        let mut yuv_buffer = vec![0u8; aligned_buffer_size];
        yuv_buffer[0..y_size].fill(16);
        yuv_buffer[y_size..yuv_size].fill(128);

        Ok(Self {
            width,
            height,
            yuv_buffer,
        })
    }

    pub fn convert(
        &mut self,
        src: &[u8],
        src_stride: usize,
    ) -> Result<&[u8], Box<dyn std::error::Error>> {
        if src_stride == 0 {
            return Err("Stride cannot be zero".into());
        }

        let width = self.width;
        let height = self.height;
        let dst_stride_y = width;
        let dst_stride_uv = width / 2;

        let y_size = width * height;
        let uv_size = (width / 2) * (height / 2);
        let uv_offset = y_size;
        let v_offset = uv_offset + uv_size;

        let dst_y = self.yuv_buffer.as_mut_ptr();
        let dst_u = self.yuv_buffer[uv_offset..].as_mut_ptr();
        let dst_v = self.yuv_buffer[v_offset..].as_mut_ptr();

        // RustDesk uses ARGBToI420 for BGRA input (libyuv treats BGRA as ARGB in little-endian)
        call_yuv!(ARGBToI420(
            src.as_ptr(),
            src_stride as _,
            dst_y,
            dst_stride_y as _,
            dst_u,
            dst_stride_uv as _,
            dst_v,
            dst_stride_uv as _,
            width as _,
            height as _,
        ));

        Ok(&self.yuv_buffer[..y_size + uv_size * 2])
    }
}
