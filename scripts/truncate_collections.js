// Switch to our database
db = db.getSiblingDB('media_metadata');

// Truncate all collections
db.models.deleteMany({});
db.content.deleteMany({});
db.model_content_map.deleteMany({});

// Print counts to verify
print("Collection counts after truncation:");
print("models:", db.models.countDocuments());
print("content:", db.content.countDocuments());
print("model_content_map:", db.model_content_map.countDocuments());
