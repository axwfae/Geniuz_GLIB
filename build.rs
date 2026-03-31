fn main(){
    println!("cargo:rustc-link-arg=-WL,-rpath,'$ORIGIN'");
}
