use lazy_static::lazy_static;
use uuid::Uuid;
// e8398181-1506-4a68-b239-20c24970080a
// fedccb84-20f4-4604-a47b-080efeb8eac1
// df14874d-211d-40b5-af49-42c6fdc4b003
// 7d9e622e-0d6c-481f-938f-5c342e6da46f
// c84d9a17-c924-42ad-99f8-9b8f59833fda
// 95770a8d-9908-4ef0-9f68-e81d79627591
// d18aca33-eb38-4cf9-b781-5a9b3fff7baa
// 559d122e-4a37-4939-9307-dc7fa59cd6f2
// eb5fa3af-4619-4ed3-b948-1faa86499cc8
// 701963bb-7737-4e95-893b-dfe4813b6258
// 049bb518-6482-466b-8e0e-00d46d39a5a0
// 3c5d3549-eb7a-48b4-bc6c-46a749f401ff
// 95680e19-3933-4ad2-ba7b-fa4b8e9109e8
// bcc85e8d-7b49-4f02-b1e4-7ca19e84b3c4
// 80bfaea3-540b-4c96-b6a3-91db69054fe9
// 0d0b2466-2405-4c9e-8d5c-9f9f9ba84642
// 8713f9e6-bf71-47a5-9090-baad58b1008b
// 4abeec9d-11bb-4046-b41c-b9166f71b1f3
// aaddf835-c0d4-4c6a-ad07-80e95cc46e11
// ed5c39af-9fb9-405b-83e4-5c5399b654da
// 9ecb5bef-5258-4cfe-aaa0-01e852818eea
// f80ce1ac-9df3-4366-92d8-3d669c374640
// 1ba7d016-1552-4d3d-9683-5e083e249d58
// 208c442e-867d-4a24-9aa9-d5108b41ec15
// 5e74548c-e385-417c-a76e-4e7c22e31262
// 866e0ed0-d1ec-4e0e-a3a5-a2fd78b4d722
// b214f3f7-3465-4efb-96d8-6bb1c4de5c7d
// 41a2792f-0414-4e15-aa4d-63a409315644
// aa4988af-659b-458e-9804-698b81ab99cb
// 98b38a66-0e07-4a77-949b-c1979a2ee808

// f80ce1ac-4619-4ed3-b948-1faa86499cc8
macro_rules! device_uuid {
    ($ident: ident, $val: tt) => {
        lazy_static! {
            pub static ref $ident: Uuid = Uuid::parse_str($val).unwrap();        
        }
    };
}

const _KERNEL_ASSIGNED_PREFIX: &str = "f80ce1ac";

device_uuid!(FRAMEBUFFER, "f80ce1ac-890f-4a92-8844-fb447d01992c");
device_uuid!(SERIAL, "f80ce1ac-7bde-4b7a-9398-ea31faff52c1");
device_uuid!(IPL, "f80ce1ac-5759-458f-bbd1-71112e971117");
device_uuid!(CPU, "f80ce1ac-d1ec-4e0e-a3a5-a2fd78b4d722");

