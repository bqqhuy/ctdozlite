//! Business, Official Account & Product Catalog (Zalo Parity C01).
//!
//! Provides data structures and API helpers to manage business profiles,
//! OA badges, and product catalogs (`/api/prodcatalog/list`, `/api/prodcatalog/item/create`).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use crate::session::Session;

/// An item in a business product catalog.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProductCatalogItem {
    pub id: String,
    pub name: String,
    pub price: u64,
    pub description: String,
    pub photos: Vec<String>,
    pub is_active: bool,
}

/// Profile information for a Business / Official Account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BusinessProfile {
    pub uid: String,
    pub display_name: String,
    pub category: String,
    pub is_verified_oa: bool,
    pub description: String,
}

/// Parse a list of product catalog items from Zalo API response.
pub fn parse_product_catalog(val: &Value) -> Vec<ProductCatalogItem> {
    let data = val.get("data").unwrap_or(val);
    let items_arr = data
        .get("items")
        .or_else(|| data.get("products"))
        .or_else(|| data.get("catalog"))
        .and_then(|v| v.as_array());

    let Some(arr) = items_arr else {
        return Vec::new();
    };

    arr.iter()
        .filter_map(|it| {
            let id = it.get("id").or_else(|| it.get("product_id"))?.as_str()?.to_string();
            let name = it.get("name").or_else(|| it.get("title")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let price = it.get("price").and_then(crate::api_response::integer).unwrap_or(0) as u64;
            let description = it.get("description").or_else(|| it.get("desc")).and_then(|v| v.as_str()).unwrap_or("").to_string();
            let photos = it.get("photos").or_else(|| it.get("images"))
                .and_then(|v| v.as_array())
                .map(|p_arr| {
                    p_arr.iter().filter_map(|p| p.as_str().map(ToString::to_string)).collect()
                })
                .unwrap_or_default();
            let is_active = it.get("status").and_then(crate::api_response::integer).map(|s| s == 1).unwrap_or(true);

            Some(ProductCatalogItem {
                id,
                name,
                price,
                description,
                photos,
                is_active,
            })
        })
        .collect()
}

/// Call `/api/prodcatalog/list` to fetch the account's product catalog.
pub async fn fetch_product_catalog(s: &Session) -> Result<Vec<ProductCatalogItem>> {
    let url = format!("{}/api/prodcatalog/list", s.domains.profile);
    let payload = json!({
        "page": 1,
        "count": 50,
        "imei": s.imei,
    });
    let params = s.encoded_params(&payload)
        .context("Mã hoá tham số lấy danh mục sản phẩm thất bại")?;
    let resp = s.post_form(&url, format!("params={params}")).await?;
    let decrypted = s.maybe_decrypt(resp, s.session_key_bytes()).await;
    Ok(parse_product_catalog(&decrypted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_product_catalog() {
        let val = json!({
            "error_code": 0,
            "data": {
                "items": [
                    {
                        "id": "prod_1001",
                        "name": "Áo Thun Zalo Developer",
                        "price": 250000,
                        "description": "Chất liệu cotton cao cấp 100%",
                        "photos": ["https://photo.zdn.vn/tshirt1.jpg"],
                        "status": 1
                    }
                ]
            }
        });

        let catalog = parse_product_catalog(&val);
        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog[0].id, "prod_1001");
        assert_eq!(catalog[0].name, "Áo Thun Zalo Developer");
        assert_eq!(catalog[0].price, 250000);
        assert!(catalog[0].is_active);
    }

    #[test]
    fn test_business_profile_verified_badge() {
        let profile = BusinessProfile {
            uid: "oa_998877".into(),
            display_name: "Zalo Lite Official".into(),
            category: "Công nghệ thông tin".into(),
            is_verified_oa: true,
            description: "Trang hỗ trợ chính thức ctdozlite".into(),
        };

        assert!(profile.is_verified_oa);
        assert_eq!(profile.display_name, "Zalo Lite Official");
    }
}
