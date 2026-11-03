//! Bounded, credential-free WebAssembly host for the official sync cryptographic ABI.
//! Only memory, typed arrays and OS randomness are exposed; no JavaScript, files or networking.
use anyhow::{bail, Context, Result};
use wasmi::{
    Caller, Engine, Extern, Instance, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val,
};
use zeroize::{Zeroize, Zeroizing};

const MEMORY_LIMIT: usize = 64 * 1024 * 1024;
const OUTPUT_LIMIT: usize = 16 * 1024 * 1024;
const FUEL: u64 = 800_000_000;

#[derive(Clone)]
enum Object {
    Undefined,
    Global,
    Crypto,
    Function,
    Memory,
    View(usize, usize),
    Bytes(Zeroizing<Vec<u8>>),
    Plain,
    Array(usize),
    String,
}
struct Host {
    objects: Vec<Object>,
    limits: StoreLimits,
}
impl Host {
    fn insert(&mut self, value: Object) -> Result<i32, wasmi::Error> {
        let allocated: usize = self
            .objects
            .iter()
            .map(|o| match o {
                Object::Bytes(b) => b.len(),
                _ => 0,
            })
            .sum();
        let requested = match &value {
            Object::Bytes(b) => b.len(),
            _ => 0,
        };
        if allocated.saturating_add(requested) > MEMORY_LIMIT {
            return Err(error());
        }
        if let Some(i) = self
            .objects
            .iter()
            .enumerate()
            .skip(132)
            .find_map(|(i, v)| matches!(v, Object::Undefined).then_some(i))
        {
            self.objects[i] = value;
            return Ok(i as i32);
        }
        if self.objects.len() >= 4096 {
            return Err(wasmi::Error::new("codec host object limit"));
        }
        self.objects.push(value);
        Ok((self.objects.len() - 1) as i32)
    }
    fn object(&self, id: i32) -> Object {
        self.objects
            .get(id as usize)
            .cloned()
            .unwrap_or(Object::Undefined)
    }
    fn release(&mut self, id: i32) {
        if id >= 132 {
            if let Some(v) = self.objects.get_mut(id as usize) {
                if let Object::Bytes(bytes) = v {
                    bytes.zeroize();
                }
                *v = Object::Undefined;
            }
        }
    }
}
impl Drop for Host {
    fn drop(&mut self) {
        for o in &mut self.objects {
            if let Object::Bytes(bytes) = o {
                bytes.zeroize();
            }
        }
    }
}
fn error() -> wasmi::Error {
    wasmi::Error::new("unsupported or malformed cryptographic host call")
}
fn memory(c: &Caller<'_, Host>) -> Result<wasmi::Memory, wasmi::Error> {
    c.get_export("memory")
        .and_then(Extern::into_memory)
        .ok_or_else(error)
}
fn read_object(c: &Caller<'_, Host>, o: &Object) -> Result<Vec<u8>, wasmi::Error> {
    match o {
        Object::Bytes(b) => Ok(b.to_vec()),
        Object::View(p, n) => {
            let m = memory(c)?;
            let end = p.checked_add(*n).ok_or_else(error)?;
            let b = m.data(c).get(*p..end).ok_or_else(error)?;
            if b.len() > OUTPUT_LIMIT {
                return Err(error());
            }
            Ok(b.to_vec())
        }
        _ => Err(error()),
    }
}
fn write_object(
    c: &mut Caller<'_, Host>,
    id: i32,
    offset: usize,
    b: &[u8],
) -> Result<(), wasmi::Error> {
    match c.data().object(id) {
        Object::View(p, n) => {
            if offset.checked_add(b.len()).ok_or_else(error)? > n {
                return Err(error());
            }
            memory(c)?
                .write(&mut *c, p + offset, b)
                .map_err(|_| error())
        }
        Object::Bytes(mut bytes) => {
            let end = offset.checked_add(b.len()).ok_or_else(error)?;
            bytes
                .get_mut(offset..end)
                .ok_or_else(error)?
                .copy_from_slice(b);
            if let Some(o) = c.data_mut().objects.get_mut(id as usize) {
                *o = Object::Bytes(bytes);
                Ok(())
            } else {
                Err(error())
            }
        }
        _ => Err(error()),
    }
}
fn host_call(
    name: &str,
    mut c: Caller<'_, Host>,
    args: &[Val],
    out: &mut [Val],
) -> Result<(), wasmi::Error> {
    let arg = |i: usize| args.get(i).and_then(Val::i32).ok_or_else(error);
    let mut result = 0;
    match name {
        "__wbindgen_object_drop_ref" => c.data_mut().release(arg(0)?),
        "__wbindgen_object_clone_ref" => {
            let o = c.data().object(arg(0)?);
            result = c.data_mut().insert(o)?;
        }
        "__wbindgen_memory" => result = c.data_mut().insert(Object::Memory)?,
        "__wbindgen_is_object" => {
            result = matches!(
                c.data().object(arg(0)?),
                Object::Global
                    | Object::Crypto
                    | Object::Memory
                    | Object::View(..)
                    | Object::Bytes(..)
                    | Object::Plain
                    | Object::Array(..)
            ) as i32
        }
        "__wbindgen_is_string" => {
            result = matches!(c.data().object(arg(0)?), Object::String) as i32
        }
        "__wbindgen_is_function" => {
            result = matches!(c.data().object(arg(0)?), Object::Function) as i32
        }
        "__wbindgen_is_undefined" => {
            result = matches!(c.data().object(arg(0)?), Object::Undefined) as i32
        }
        "__wbindgen_string_new" => {
            let p = arg(0)? as usize;
            let n = arg(1)? as usize;
            if n > OUTPUT_LIMIT || p.checked_add(n).ok_or_else(error)? > memory(&c)?.data(&c).len()
            {
                return Err(error());
            }
            result = c.data_mut().insert(Object::String)?;
        }
        "__wbindgen_number_new" => result = c.data_mut().insert(Object::Plain)?,
        "__wbindgen_throw" => return Err(wasmi::Error::new("cryptographic module rejected input")),
        "__wbg_new_405e22f390576ce2" | "__wbg_new_8a6f238a6ece86ea" => {
            result = c.data_mut().insert(Object::Plain)?
        }
        "__wbg_new_78feb108b6472713" => result = c.data_mut().insert(Object::Array(0))?,
        "__wbg_set_3f1d0b984ed272ed" => {
            c.data_mut().release(arg(1)?);
            c.data_mut().release(arg(2)?);
        }
        "__wbg_push_737cfc8c1432c2c6" => {
            let id = arg(0)?;
            if let Some(Object::Array(n)) = c.data_mut().objects.get_mut(id as usize) {
                *n += 1;
                result = *n as i32;
            }
            c.data_mut().release(arg(1)?);
        }
        "__wbg_buffer_609cc3eee51ed158" => result = c.data_mut().insert(Object::Memory)?,
        "__wbg_newwithbyteoffsetandlength_d97e637ebe145a9a" => {
            let p = arg(1)? as usize;
            let n = arg(2)? as usize;
            if n > OUTPUT_LIMIT || p.checked_add(n).ok_or_else(error)? > memory(&c)?.data(&c).len()
            {
                return Err(error());
            }
            result = c.data_mut().insert(Object::View(p, n))?;
        }
        "__wbg_new_a12002a7f91c75be" => {
            let o = c.data().object(arg(0)?);
            let bytes = if matches!(o, Object::Memory) {
                Object::View(0, memory(&c)?.data(&c).len())
            } else {
                Object::Bytes(Zeroizing::new(read_object(&c, &o)?))
            };
            result = c.data_mut().insert(bytes)?;
        }
        "__wbg_newwithlength_a381634e90c276d4" => {
            let n = arg(0)? as usize;
            if n > OUTPUT_LIMIT {
                return Err(error());
            }
            result = c
                .data_mut()
                .insert(Object::Bytes(Zeroizing::new(vec![0; n])))?;
        }
        "__wbg_subarray_aa9065fa9dc5df96" => {
            let o = c.data().object(arg(0)?);
            let from = arg(1)? as usize;
            let to = arg(2)? as usize;
            let o = match o {
                Object::View(p, n) if from <= to && to <= n => Object::View(p + from, to - from),
                Object::Bytes(bytes) => Object::Bytes(Zeroizing::new(
                    bytes.get(from..to).ok_or_else(error)?.to_vec(),
                )),
                _ => return Err(error()),
            };
            result = c.data_mut().insert(o)?;
        }
        "__wbg_set_65595bdd868b3009" => {
            let bytes = Zeroizing::new(read_object(&c, &c.data().object(arg(1)?))?);
            write_object(&mut c, arg(0)?, arg(2)? as usize, &bytes)?;
        }
        "__wbg_getRandomValues_bcb4912f16000dc4" | "__wbg_randomFillSync_ab2cfe79ebbf2740" => {
            let id = arg(1)?;
            let o = c.data().object(id);
            let n = match o {
                Object::View(_, n) => n,
                Object::Bytes(ref b) => b.len(),
                _ => return Err(error()),
            };
            if n > 65536 {
                return Err(error());
            }
            let mut bytes = Zeroizing::new(vec![0; n]);
            crate::random::fill(&mut bytes).map_err(|_| error())?;
            write_object(&mut c, id, 0, &bytes)?;
        }
        "__wbg_crypto_ed58b8e10a292839" | "__wbg_msCrypto_0a36e2ec3a343d26" => {
            result = c.data_mut().insert(Object::Crypto)?
        }
        "__wbg_process_5c1d670bc53614b8"
        | "__wbg_versions_c71aa1626a93e0a1"
        | "__wbg_node_02999533c4ea02e3" => result = 128,
        "__wbg_require_79b1e9274cde3c87" | "__wbg_newnoargs_105ed471475aaf50" => {
            result = c.data_mut().insert(Object::Function)?
        }
        "__wbg_call_672a4d21634d4a24" | "__wbg_call_7cccdd69e0791ae2" => {
            result = c.data_mut().insert(Object::Global)?
        }
        "__wbg_static_accessor_GLOBAL_88a902d13a557d07"
        | "__wbg_static_accessor_GLOBAL_THIS_56578be7e9f832b0"
        | "__wbg_static_accessor_WINDOW_5de37043a91a9c40"
        | "__wbg_static_accessor_SELF_37c5d418e4bf5819" => {
            result = c.data_mut().insert(Object::Global)?
        }
        "__wbg_error_7534b8e9a36f1ab4" => {
            let free = c
                .get_export("__wbindgen_export_1")
                .and_then(Extern::into_func)
                .ok_or_else(error)?;
            free.call(
                &mut c,
                &[Val::I32(arg(0)?), Val::I32(arg(1)?), Val::I32(1)],
                &mut [],
            )?;
        }
        "__wbg_stack_0ed75d68575b0f3c" => memory(&c)?
            .write(&mut c, arg(0)? as usize, &[0; 8])
            .map_err(|_| error())?,
        "__wbg_apply_36be6a55257c99bf" => result = 128,
        _ => return Err(error()),
    }
    if let Some(v) = out.first_mut() {
        *v = Val::I32(result);
    }
    Ok(())
}

/// Zeroed byte buffer used for transient cryptographic material.
pub type SecretBytes = Zeroizing<Vec<u8>>;

/// In-memory transfer keys. Private material is never formatted or persisted.
pub struct KeyPair {
    /// Protocol public key including the 0x05 key-type prefix.
    pub public: Vec<u8>,
    /// Ephemeral private key, zeroed on drop.
    pub private: Zeroizing<Vec<u8>>,
}
/// Isolated official cryptographic module, with memory and instruction budgets.
pub struct Codec {
    store: Store<Host>,
    instance: Instance,
}
impl Codec {
    /// Instantiate a verified module without exposing files, process execution or network.
    /// # Errors
    /// Rejects invalid modules, unsupported imports and resource exhaustion.
    pub fn new(bytes: &[u8]) -> Result<Self> {
        super::assets::verify(bytes)?;
        anyhow::ensure!(
            bytes.len() <= 1024 * 1024,
            "Cryptographic module exceeds limit"
        );
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        let engine = Engine::new(&config);
        let module = Module::new(&engine, bytes)?;
        let mut linker = Linker::new(&engine);
        for import in module.imports() {
            anyhow::ensure!(
                import.module() == "wbg",
                "Unsupported cryptographic module import"
            );
            let ty = import
                .ty()
                .func()
                .context("Unsupported non-function import")?
                .clone();
            let name = import.name().to_owned();
            let callback_name = name.clone();
            linker.func_new("wbg", &name, ty, move |c, args, out| {
                host_call(&callback_name, c, args, out)
            })?;
        }
        let mut store = Store::new(
            &engine,
            Host {
                objects: vec![Object::Undefined; 132],
                limits: StoreLimitsBuilder::new()
                    .memory_size(MEMORY_LIMIT)
                    .memories(1)
                    .instances(1)
                    .build(),
            },
        );
        store.limiter(|host| &mut host.limits);
        store.set_fuel(FUEL)?;
        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;
        Ok(Self { store, instance })
    }
    fn call(&mut self, name: &str, args: &[i32], result: bool) -> Result<i32> {
        self.store.set_fuel(FUEL)?;
        let function = self
            .instance
            .get_func(&self.store, name)
            .context("Cryptographic ABI changed")?;
        let args: Vec<Val> = args.iter().copied().map(Val::I32).collect();
        let mut out = if result { vec![Val::I32(0)] } else { vec![] };
        function
            .call(&mut self.store, &args, &mut out)
            .map_err(|_| anyhow::anyhow!("Cryptographic operation rejected or exceeded budget"))?;
        Ok(out.first().and_then(Val::i32).unwrap_or(0))
    }
    fn mem(&self) -> Result<wasmi::Memory> {
        self.instance
            .get_memory(&self.store, "memory")
            .context("Codec memory missing")
    }
    fn words(&self, ptr: i32) -> Result<[i32; 4]> {
        let mut bytes = [0; 16];
        self.mem()?.read(&self.store, ptr as usize, &mut bytes)?;
        Ok(std::array::from_fn(|i| {
            i32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().expect("fixed word"))
        }))
    }
    fn alloc(&mut self, bytes: &[u8]) -> Result<(i32, i32)> {
        anyhow::ensure!(bytes.len() <= OUTPUT_LIMIT, "Codec input exceeds limit");
        let ptr = self.call("__wbindgen_export_2", &[bytes.len() as i32, 1], true)?;
        self.mem()?.write(&mut self.store, ptr as usize, bytes)?;
        Ok((ptr, bytes.len() as i32))
    }
    fn bytes_field(&mut self, name: &str, object: i32) -> Result<Zeroizing<Vec<u8>>> {
        let id = self.call(name, &[object], true)?;
        let o = self.store.data().object(id);
        let bytes = match o {
            Object::Bytes(bytes) => bytes.to_vec(),
            Object::View(ptr, len) => {
                anyhow::ensure!(len <= OUTPUT_LIMIT, "Codec output exceeds limit");
                let mut bytes = vec![0; len];
                self.mem()?.read(&self.store, ptr, &mut bytes)?;
                bytes
            }
            _ => bail!("Codec returned a non-byte value"),
        };
        self.store.data_mut().release(id);
        Ok(Zeroizing::new(bytes))
    }
    fn returned_object(&mut self, name: &str, inputs: &[&[u8]], extra: &[i32]) -> Result<i32> {
        let stack = self.call("__wbindgen_add_to_stack_pointer", &[-16], true)?;
        let result = (|| {
            let mut args = vec![stack];
            for input in inputs {
                let (p, n) = self.alloc(input)?;
                args.extend([p, n]);
            }
            args.extend(extra);
            self.call(name, &args, false)?;
            let words = self.words(stack)?;
            anyhow::ensure!(words[2] == 0, "Cryptographic authentication failed");
            Ok(words[0])
        })();
        let restore = self.call("__wbindgen_add_to_stack_pointer", &[16], true);
        result.and_then(|value| restore.map(|_| value))
    }
    /// Generate fresh transfer keys using Windows CNG / operating-system entropy.
    /// # Errors
    /// Returns module/entropy failures or resource exhaustion.
    pub fn key_pair(&mut self) -> Result<KeyPair> {
        let object = self.returned_object("zprotoEd25519GenerateKeyPair", &[], &[])?;
        let result = (|| {
            Ok(KeyPair {
                public: self
                    .bytes_field("zprotoed25519keypair_publicKey", object)?
                    .to_vec(),
                private: self.bytes_field("zprotoed25519keypair_privateKey", object)?,
            })
        })();
        let freed = self.call("__wbg_zprotoed25519keypair_free", &[object, 0], false);
        result.and_then(|pair| freed.map(|_| pair))
    }
    /// Authenticate a metadata chunk and obtain its ephemeral session record.
    /// # Errors
    /// Rejects malformed data, authentication failures and exhausted budgets.
    pub fn metadata(
        &mut self,
        chunk: &[u8],
        identity: &KeyPair,
        ephemeral: &KeyPair,
    ) -> Result<(SecretBytes, SecretBytes)> {
        let object = self.returned_object(
            "zprotoSync2DecryptMetadata",
            &[
                chunk,
                &identity.public,
                &identity.private,
                &ephemeral.private,
            ],
            &[0, 0],
        )?;
        let result = (|| {
            Ok((
                self.bytes_field("zprotosyncmetadatadecrypted_plaintext", object)?,
                self.bytes_field("zprotosyncmetadatadecrypted_sessionRecord", object)?,
            ))
        })();
        let freed = self.call(
            "__wbg_zprotosyncmetadatadecrypted_free",
            &[object, 0],
            false,
        );
        result.and_then(|v| freed.map(|_| v))
    }
    /// Authenticate a message chunk against the matching metadata session.
    /// # Errors
    /// Rejects malformed data, authentication failures and exhausted budgets.
    pub fn message(&mut self, chunk: &[u8], record: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
        let object = self.returned_object("zprotoSync2DecryptMessage", &[chunk, record], &[])?;
        let result = self.bytes_field("zprotosyncmessagedecrypted_plaintext", object);
        let freed = self.call("__wbg_zprotosyncmessagedecrypted_free", &[object, 0], false);
        result.and_then(|v| freed.map(|_| v))
    }
}
impl Drop for Codec {
    fn drop(&mut self) {
        if let Some(memory) = self.instance.get_memory(&self.store, "memory") {
            memory.data_mut(&mut self.store).zeroize();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the verified public codec asset; CI supplies CTDOZ_CODEC_TEST_PATH"]
    fn official_codec_interoperability() {
        let path = std::env::var("CTDOZ_CODEC_TEST_PATH").expect("codec asset path");
        let mut codec = Codec::new(&std::fs::read(path).unwrap()).unwrap();
        use base64::Engine;
        let vector: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/phone_sync_crypto_synthetic.json"
        ))
        .unwrap();
        let b64 = base64::engine::general_purpose::STANDARD;
        let bytes = |key: &str| b64.decode(vector[key].as_str().unwrap()).unwrap();
        let identity = KeyPair {
            public: bytes("ik_public"),
            private: Zeroizing::new(bytes("ik_private")),
        };
        let ephemeral = KeyPair {
            public: bytes("ek_public"),
            private: Zeroizing::new(bytes("ek_private")),
        };
        let (plain, record) = codec
            .metadata(&bytes("metadata"), &identity, &ephemeral)
            .unwrap();
        assert_eq!(&*plain, b"metadata fixture");
        assert_eq!(
            &*codec.message(&bytes("message"), &record).unwrap(),
            b"message fixture"
        );
        let a = codec.key_pair().unwrap();
        let b = codec.key_pair().unwrap();
        assert_eq!(a.public.len(), 33);
        assert_eq!(a.private.len(), 32);
        assert_ne!(a.public, b.public);
        let sender = codec.key_pair().unwrap();
        let object = codec
            .returned_object(
                "zprotoSync2CreateMetadataCipher",
                &[
                    b"metadata fixture",
                    &sender.public,
                    &sender.private,
                    &a.public,
                    &b.public,
                ],
                &[0, 0],
            )
            .unwrap();
        let ciphertext = codec
            .bytes_field("zprotosyncmetadatadecrypted_plaintext", object)
            .unwrap();
        let decrypted = codec.metadata(&ciphertext, &a, &b).unwrap();
        assert_eq!(&*decrypted.0, b"metadata fixture");
        let sender_record = codec
            .bytes_field("zprotosyncmetadatadecrypted_sessionRecord", object)
            .unwrap();
        codec
            .call(
                "__wbg_zprotosyncmetadataencrypted_free",
                &[object, 0],
                false,
            )
            .unwrap();
        let encrypted = codec
            .returned_object(
                "zprotoSync2EncryptMessage",
                &[b"message fixture", &sender_record],
                &[],
            )
            .unwrap();
        let message = codec
            .bytes_field("zprotosyncmessagedecrypted_plaintext", encrypted)
            .unwrap();
        codec
            .call(
                "__wbg_zprotosyncmessageencrypted_free",
                &[encrypted, 0],
                false,
            )
            .unwrap();
        assert_eq!(
            &*codec.message(&message, &decrypted.1).unwrap(),
            b"message fixture"
        );
        let mut tampered = message.to_vec();
        let last = tampered.len() - 1;
        tampered[last] ^= 1;
        assert!(codec.message(&tampered, &decrypted.1).is_err());
        if let Ok(path) = std::env::var("CTDOZ_WRITE_SYNTHETIC_VECTOR") {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD;
            std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({
                "source":"Synthetic fixture generated offline with the pinned official codec; no real account or user data",
                "ik_public":b64.encode(&a.public),"ik_private":b64.encode(&*a.private),
                "ek_public":b64.encode(&b.public),"ek_private":b64.encode(&*b.private),
                "metadata":b64.encode(&*ciphertext),"message":b64.encode(&*message)
            })).unwrap()).unwrap();
        }

        assert!(codec.metadata(b"invalid fixture", &a, &b).is_err());
        assert!(codec
            .message(b"invalid fixture", b"invalid record")
            .is_err());
    }
}
