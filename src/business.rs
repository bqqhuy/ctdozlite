//! Unverified business API models and fail-closed network gates.
//!
//! Native PC evidence does not establish the previous request contract; see
//! `re-notes/55-unverified-feature-gates.md`. Parsers remain offline research helpers.

use crate::session::Session;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use serde_json::json;
use serde_json::Value;

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
pub fn parse_product_catalog(val: &Value) -> Result<Vec<ProductCatalogItem>> {
    let data = crate::api_response::checked_data(val.clone())?;
    let items_arr = data
        .get("items")
        .or_else(|| data.get("products"))
        .or_else(|| data.get("catalog"))
        .and_then(|v| v.as_array());

    let arr = items_arr.context("Thiếu danh mục sản phẩm trong phản hồi")?;
    anyhow::ensure!(arr.len() <= 10000, "Danh mục sản phẩm quá lớn");

    arr.iter()
        .map(|it| {
            let id = it
                .get("id")
                .or_else(|| it.get("product_id"))
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .context("Thiếu mã sản phẩm")?
                .to_string();
            let name = it
                .get("name")
                .or_else(|| it.get("title"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let price = u64::try_from(
                it.get("price")
                    .and_then(crate::api_response::integer)
                    .context("Thiếu giá sản phẩm")?,
            )
            .context("Giá sản phẩm không hợp lệ")?;
            let description = it
                .get("description")
                .or_else(|| it.get("desc"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let photos = it
                .get("photos")
                .or_else(|| it.get("images"))
                .and_then(|v| v.as_array())
                .map(|p_arr| {
                    p_arr
                        .iter()
                        .filter_map(|p| p.as_str().map(ToString::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let is_active = it
                .get("status")
                .and_then(crate::api_response::integer)
                .map(|s| s == 1)
                .unwrap_or(true);

            Ok(ProductCatalogItem {
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

/// Strict parser for product catalog items from Zalo API response.
pub fn try_parse_product_catalog(val: &Value) -> Result<Vec<ProductCatalogItem>> {
    parse_product_catalog(val)
}

/// Return an unavailable error until this request contract has native evidence.
pub async fn fetch_product_catalog(_s: &Session) -> Result<Vec<ProductCatalogItem>> {
    // Fail closed until native protocol evidence establishes this contract.
    anyhow::bail!("Danh mục sản phẩm chưa khả dụng: chưa xác minh dịch vụ và schema Zalo PC.")
}

/// Return an unavailable error until this request contract has native evidence.
pub async fn fetch_catalog_list(_s: &Session) -> Result<Value> {
    // Fail closed until native protocol evidence establishes this contract.
    anyhow::bail!("Danh mục sản phẩm chưa khả dụng: chưa xác minh dịch vụ và schema Zalo PC.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unverified_contracts_fail_before_using_session_credentials() {
        let session = Session::new(false, false, false).unwrap();
        assert!(fetch_product_catalog(&session).await.is_err());
        assert!(fetch_catalog_list(&session).await.is_err());
    }

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

        let catalog = parse_product_catalog(&val).unwrap();
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

    #[test]
    fn missing_or_refused_catalog_is_not_an_empty_success() {
        assert!(parse_product_catalog(&json!({"error_code":600,"data":{"items":[]}})).is_err());
        assert!(parse_product_catalog(&json!({"error_code":0,"data":{}})).is_err());
        assert!(parse_product_catalog(
            &json!({"error_code":114,"error_message":"Permission denied","data":null})
        )
        .is_err());
        assert!(parse_product_catalog(
            &json!({"error_code":0,"data":{"items":[{"id":"x","price":-1}]}})
        )
        .is_err());
        assert!(
            parse_product_catalog(&json!({"error_code":0,"data":{"items":[]}}))
                .unwrap()
                .is_empty()
        );
        assert!(
            try_parse_product_catalog(&json!({"error_code":0,"data":{"items":[]}}))
                .unwrap()
                .is_empty()
        );
    }
}
