use image::AnimationDecoder;
fn main() {
 let bytes=std::fs::read("src/assets/ami_fat_cat/ami_fat_cat_02.gif").unwrap();
 let frames=image::codecs::gif::GifDecoder::new(std::io::Cursor::new(bytes)).unwrap().into_frames().collect_frames().unwrap();
 let width=frames[0].buffer().width() as usize; let height=frames[0].buffer().height() as usize;
 let total_width=width*frames.len();
 println!("production geometry: {} frames, {}x{}, bytes={}",frames.len(),total_width,height,total_width*height*4);
 let ctx=egui::Context::default();
 let _=ctx.run(egui::RawInput{max_texture_side:Some(8192),..Default::default()},|_|{});
 let res=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ctx.load_texture("stk_99901_999002",egui::ColorImage::new([total_width,height],egui::Color32::WHITE),Default::default())));
 println!("oversized texture panic reproduced: {}",res.is_err()); assert!(res.is_err());
}
